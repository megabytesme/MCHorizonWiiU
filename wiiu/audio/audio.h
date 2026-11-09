#pragma once

#include <stdint.h>

void *horizon_wiiu_audio_open(uint32_t *frames, uint32_t *rate);
void horizon_wiiu_audio_submit(void *handle, const float *stereo, uint32_t frames);
void horizon_wiiu_audio_close(void *handle);
