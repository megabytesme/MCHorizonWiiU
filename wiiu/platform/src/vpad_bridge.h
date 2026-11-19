#pragma once
#include <stdint.h>

typedef struct HorizonVpadSample {
   uint32_t buttons;
   float left_x, left_y, right_x, right_y;
   /* Calibrated coordinates are valid only when touch_valid is nonzero. */
   uint16_t touch_x, touch_y;
   /* Physical contact and calibrated-point validity are independent. */
   uint32_t touch_active, touch_valid;
} HorizonVpadSample;

/* Ordered button states observed in one VPADRead batch. */
#define HORIZON_VPAD_SAMPLE_CAPACITY 16
#define HORIZON_VPAD_HISTORY_CAPACITY (HORIZON_VPAD_SAMPLE_CAPACITY * 3)

static inline int32_t
horizon_vpad_uses_tight_button_mode(int32_t swkbd_open, int32_t swkbd_occupied)
{
   return swkbd_open == 0 && swkbd_occupied == 0;
}

typedef struct HorizonVpadButtonSample {
   uint32_t hold;
   uint32_t trigger;
   uint32_t release;
} HorizonVpadButtonSample;

/* Convert WUT's newest-first samples into chronological button states. */
static inline uint32_t
horizon_vpad_collect_button_history(const HorizonVpadButtonSample *newest_first,
                                    uint32_t count,
                                    uint32_t previous, uint32_t *out,
                                    uint32_t capacity)
{
   uint32_t written = 0;
   while (count > 0) {
      const HorizonVpadButtonSample *sample = &newest_first[--count];
      uint32_t pressed = previous | sample->trigger;
      if (pressed != previous && written < capacity) out[written++] = pressed;
      uint32_t released = pressed & ~sample->release;
      if (released != pressed && written < capacity) out[written++] = released;
      if (sample->hold != released && written < capacity) out[written++] = sample->hold;
      previous = sample->hold;
   }
   return written;
}

typedef struct HorizonVpadBatch {
   HorizonVpadSample latest;
   uint32_t transition_count;
   uint32_t transitions[HORIZON_VPAD_HISTORY_CAPACITY];
} HorizonVpadBatch;

/* The caller retains its last sample when no new VPAD sample is available. */
_Static_assert(sizeof(HorizonVpadSample) == 32, "unexpected VPAD bridge layout");
_Static_assert(sizeof(HorizonVpadBatch) == 228, "unexpected VPAD batch layout");

/* Returns 1 for a sample, VPAD_READ_NO_SAMPLES for no sample, or a VPAD error. */
int32_t horizon_wiiu_vpad_read(HorizonVpadBatch *batch);

/* True while CafeOS swkbd owns the current VPAD sample, including the frame
   on which the user accepts or cancels it. */
int32_t horizon_wiiu_swkbd_input_occupied(void);
