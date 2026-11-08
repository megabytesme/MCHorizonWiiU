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

/* The caller retains its last sample when no new VPAD sample is available. */
_Static_assert(sizeof(HorizonVpadSample) == 32, "unexpected VPAD bridge layout");

/* Returns 1 for a sample, VPAD_READ_NO_SAMPLES for no sample, or a VPAD error. */
int32_t horizon_wiiu_vpad_read(HorizonVpadSample *sample);
