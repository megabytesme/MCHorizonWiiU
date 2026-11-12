#include "audio.h"

#include <coreinit/cache.h>
#include <coreinit/memdefaultheap.h>
#include <coreinit/thread.h>
#include <sndcore2/core.h>
#include <sndcore2/device.h>
#include <sndcore2/voice.h>
#include <stdlib.h>
#include <string.h>

/* The Rust worker feeds this fixed SPSC queue. AX's app-frame callback only
 * copies a ready block (or silence) two frames ahead into the DSP ring. */
#define HORIZON_AUDIO_RING_FRAMES 16u
#define HORIZON_AUDIO_MAX_FRAME_SAMPLES 256u
#define HORIZON_AUDIO_QUEUE_FRAMES 32u
/* AX priority 0 is free; persistent mixer voices use the no-drop priority. */
#define HORIZON_AUDIO_AX_PRIORITY 31u
#define HORIZON_AUDIO_MAX_OUTPUT_CHANNELS 6u

typedef struct HorizonWiiUAudio {
   AXVoice *left_voice;
   AXVoice *right_voice;
   int16_t *left_ring;
   int16_t *right_ring;
   int16_t queue[HORIZON_AUDIO_QUEUE_FRAMES][HORIZON_AUDIO_MAX_FRAME_SAMPLES][2];
   uint32_t samples_per_frame;
   uint32_t samples_per_second;
   uint32_t ring_samples;
   uint32_t ring_cursor;
   volatile uint32_t queue_write;
   volatile uint32_t queue_read;
   BOOL owns_ax;
   BOOL callback_registered;
} HorizonWiiUAudio;

static HorizonWiiUAudio *sAudio;
static volatile uint32_t sCallbacksActive;

static int16_t float_to_pcm16(float value)
{
   if (value != value) {
      return 0;
   }
   if (value > 1.0f) {
      value = 1.0f;
   } else if (value < -1.0f) {
      value = -1.0f;
   }
   return (int16_t)(value * 32767.0f);
}

static void write_ring(int16_t *ring, uint32_t start, const int16_t *stereo,
                       uint32_t channel, uint32_t frames, uint32_t ring_samples)
{
   for (uint32_t i = 0; i < frames; ++i) {
      ring[(start + i) % ring_samples] = stereo[i * 2u + channel];
   }
   /* DSP reads the same physical memory through its cache-coherent mapping. */
   DCFlushRange(ring, ring_samples * sizeof(*ring));
}

static void configure_voice(AXVoice *voice, int16_t *samples, uint32_t count,
                            AXDeviceType device, uint32_t output_channel)
{
   AXVoiceOffsets offsets = {
      .dataType = AX_VOICE_FORMAT_LPCM16,
      .loopingEnabled = AX_VOICE_LOOP_ENABLED,
      .loopOffset = 0,
      .endOffset = count - 1u,
      .currentOffset = 0,
      .data = samples,
   };
   /* AX expects one four-bus mix entry for each output channel. */
   AXVoiceDeviceMixData mix[HORIZON_AUDIO_MAX_OUTPUT_CHANNELS] = { 0 };
   mix[output_channel].bus[0].volume = 0x8000u;

   AXSetVoiceOffsets(voice, &offsets);
   AXSetVoiceSrcType(voice, AX_VOICE_SRC_TYPE_LINEAR);
   AXSetVoiceSrcRatio(voice, 1.0f);
   AXSetVoiceDeviceMix(voice, device, 0, mix);
   AXSetVoiceState(voice, AX_VOICE_STATE_PLAYING);
}

static void audio_frame_callback(void)
{
   __atomic_add_fetch(&sCallbacksActive, 1u, __ATOMIC_ACQUIRE);
   HorizonWiiUAudio *audio = __atomic_load_n(&sAudio, __ATOMIC_ACQUIRE);
   if (audio == NULL) {
      __atomic_sub_fetch(&sCallbacksActive, 1u, __ATOMIC_RELEASE);
      return;
   }
   if (audio->samples_per_frame > HORIZON_AUDIO_MAX_FRAME_SAMPLES ||
       audio->left_voice == NULL || audio->right_voice == NULL) {
      __atomic_sub_fetch(&sCallbacksActive, 1u, __ATOMIC_RELEASE);
      return;
   }

   int16_t silence[HORIZON_AUDIO_MAX_FRAME_SAMPLES * 2u] = { 0 };
   uint32_t read = __atomic_load_n(&audio->queue_read, __ATOMIC_RELAXED);
   const uint32_t write = __atomic_load_n(&audio->queue_write, __ATOMIC_ACQUIRE);
   if (write - read > 4u) {
      /* Keep at most ~12 ms queued after a worker scheduling hiccup. */
      read = write - 4u;
   }
   const int16_t *stereo = silence;
   if (read != write) {
      stereo = &audio->queue[read % HORIZON_AUDIO_QUEUE_FRAMES][0][0];
   }
   const uint32_t write_at = (audio->ring_cursor + 2u * audio->samples_per_frame) % audio->ring_samples;
   write_ring(audio->left_ring, write_at, stereo, 0, audio->samples_per_frame,
              audio->ring_samples);
   write_ring(audio->right_ring, write_at, stereo, 1, audio->samples_per_frame,
              audio->ring_samples);
   if (read != write) {
      __atomic_store_n(&audio->queue_read, read + 1u, __ATOMIC_RELEASE);
   }
   audio->ring_cursor = (audio->ring_cursor + audio->samples_per_frame) % audio->ring_samples;

   __atomic_sub_fetch(&sCallbacksActive, 1u, __ATOMIC_RELEASE);
}

static void close_partial(HorizonWiiUAudio *audio)
{
   if (audio->right_voice != NULL) {
      AXSetVoiceState(audio->right_voice, AX_VOICE_STATE_STOPPED);
      AXFreeVoice(audio->right_voice);
      audio->right_voice = NULL;
   }
   if (audio->left_voice != NULL) {
      AXSetVoiceState(audio->left_voice, AX_VOICE_STATE_STOPPED);
      AXFreeVoice(audio->left_voice);
      audio->left_voice = NULL;
   }
   if (audio->right_ring != NULL) {
      MEMFreeToDefaultHeap(audio->right_ring);
      audio->right_ring = NULL;
   }
   if (audio->left_ring != NULL) {
      MEMFreeToDefaultHeap(audio->left_ring);
      audio->left_ring = NULL;
   }
   if (audio->owns_ax) {
      AXQuit();
      audio->owns_ax = FALSE;
   }
}

void *horizon_wiiu_audio_open(uint32_t *frames, uint32_t *rate)
{
   if (frames == NULL || rate == NULL || sAudio != NULL) {
      return NULL;
   }

   HorizonWiiUAudio *audio = (HorizonWiiUAudio *)calloc(1, sizeof(*audio));
   if (audio == NULL) {
      return NULL;
   }
   if (!AXIsInit()) {
      AXInit();
      audio->owns_ax = TRUE;
   }
   audio->samples_per_frame = AXGetInputSamplesPerFrame();
   audio->samples_per_second = AXGetInputSamplesPerSec();
   if (audio->samples_per_frame == 0 ||
       audio->samples_per_frame > HORIZON_AUDIO_MAX_FRAME_SAMPLES ||
       audio->samples_per_second == 0) {
      close_partial(audio);
      free(audio);
      return NULL;
   }
   *frames = audio->samples_per_frame;
   *rate = audio->samples_per_second;
   audio->ring_samples = audio->samples_per_frame * HORIZON_AUDIO_RING_FRAMES;
   const uint32_t ring_bytes = audio->ring_samples * sizeof(int16_t);
   audio->left_ring = (int16_t *)MEMAllocFromDefaultHeapEx(ring_bytes, 64);
   audio->right_ring = (int16_t *)MEMAllocFromDefaultHeapEx(ring_bytes, 64);
   if (audio->left_ring == NULL || audio->right_ring == NULL) {
      close_partial(audio);
      free(audio);
      return NULL;
   }
   memset(audio->left_ring, 0, ring_bytes);
   memset(audio->right_ring, 0, ring_bytes);
   DCFlushRange(audio->left_ring, ring_bytes);
   DCFlushRange(audio->right_ring, ring_bytes);

   audio->left_voice = AXAcquireVoice(HORIZON_AUDIO_AX_PRIORITY, NULL, NULL);
   audio->right_voice = AXAcquireVoice(HORIZON_AUDIO_AX_PRIORITY, NULL, NULL);
   if (audio->left_voice == NULL || audio->right_voice == NULL) {
      close_partial(audio);
      free(audio);
      return NULL;
   }
   configure_voice(audio->left_voice, audio->left_ring, audio->ring_samples,
                   AX_DEVICE_TYPE_TV, 0);
   configure_voice(audio->right_voice, audio->right_ring, audio->ring_samples,
                   AX_DEVICE_TYPE_TV, 1);
   configure_voice(audio->left_voice, audio->left_ring, audio->ring_samples,
                   AX_DEVICE_TYPE_DRC, 0);
   configure_voice(audio->right_voice, audio->right_ring, audio->ring_samples,
                   AX_DEVICE_TYPE_DRC, 1);

   __atomic_store_n(&sAudio, audio, __ATOMIC_RELEASE);
   if (AXRegisterAppFrameCallback(audio_frame_callback) != AX_RESULT_SUCCESS) {
      __atomic_store_n(&sAudio, NULL, __ATOMIC_RELEASE);
      while (__atomic_load_n(&sCallbacksActive, __ATOMIC_ACQUIRE) != 0) {
         OSSleepTicks(1);
      }
      close_partial(audio);
      free(audio);
      return NULL;
   }
   audio->callback_registered = TRUE;
   return audio;
}

void horizon_wiiu_audio_submit(void *handle, const float *stereo, uint32_t frames)
{
   HorizonWiiUAudio *audio = (HorizonWiiUAudio *)handle;
   if (audio == NULL || stereo == NULL || frames != audio->samples_per_frame) {
      return;
   }
   const uint32_t write = __atomic_load_n(&audio->queue_write, __ATOMIC_RELAXED);
   const uint32_t read = __atomic_load_n(&audio->queue_read, __ATOMIC_ACQUIRE);
   if (write - read >= HORIZON_AUDIO_QUEUE_FRAMES) {
      return;
   }
   int16_t (*block)[2] = audio->queue[write % HORIZON_AUDIO_QUEUE_FRAMES];
   for (uint32_t i = 0; i < frames; ++i) {
      block[i][0] = float_to_pcm16(stereo[i * 2u]);
      block[i][1] = float_to_pcm16(stereo[i * 2u + 1u]);
   }
   __atomic_store_n(&audio->queue_write, write + 1u, __ATOMIC_RELEASE);
}

void horizon_wiiu_audio_close(void *handle)
{
   HorizonWiiUAudio *audio = (HorizonWiiUAudio *)handle;
   if (audio == NULL || audio != __atomic_load_n(&sAudio, __ATOMIC_ACQUIRE)) {
      return;
   }
   if (audio->callback_registered) {
      AXDeregisterAppFrameCallback(audio_frame_callback);
      audio->callback_registered = FALSE;
   }
   __atomic_store_n(&sAudio, NULL, __ATOMIC_RELEASE);
   while (__atomic_load_n(&sCallbacksActive, __ATOMIC_ACQUIRE) != 0) {
      OSSleepTicks(1);
   }
   close_partial(audio);
   free(audio);
}
