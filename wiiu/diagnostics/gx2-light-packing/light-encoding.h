#pragma once

#include <stdint.h>

static float centered_light(uint16_t light)
{
   unsigned block = light >> 8;
   unsigned sky = light & 255;
   if (block > 240) block = 240;
   if (sky > 240) sky = 240;
   return (float)((block + 8) * 256 + sky + 8) * (1.0f / 256.0f);
}
