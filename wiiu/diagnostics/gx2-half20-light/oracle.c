#include "exact-half.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>

static uint16_t be16(const uint8_t b[2])
{
   return (uint16_t)((unsigned)b[0] << 8 | b[1]);
}

static void light_uv(uint16_t packed, float out[2])
{
   const unsigned high = packed >> 8, low = packed & 255;
   out[0] = ((high > 240 ? 240 : high) + 8) / 256.0f;
   out[1] = ((low > 240 ? 240 : low) + 8) / 256.0f;
}

int main(void)
{
   unsigned lane_bad = 0, swap_bad = 0, clamp_bad = 0;
   uint16_t half_z;
   assert(exact_half_position(0.46875f, &half_z) && half_z == 0x3780);
   for (unsigned light = 0; light <= UINT16_MAX; ++light) {
      const uint8_t gcn[4] = {half_z >> 8, half_z & 255, light >> 8, light & 255};
      const float ordinary = (float)light;
      const float gpu_pair_g = (float)be16(gcn + 2);
      const float gpu_scalar = (float)be16(gcn + 2);
      assert(memcmp(&ordinary, &gpu_pair_g, sizeof(float)) == 0);
      assert(memcmp(&ordinary, &gpu_scalar, sizeof(float)) == 0);
      float stock[2], chosen[2], lane[2], swap[2], clamp[2];
      light_uv((uint16_t)ordinary, stock);
      light_uv((uint16_t)gpu_pair_g, chosen);
      light_uv(be16(gcn), lane);
      const uint8_t reversed[2] = {gcn[3], gcn[2]};
      light_uv(be16(reversed), swap);
      const uint8_t clipped[2] = {gcn[2] > 240 ? 112 : gcn[2], gcn[3] > 240 ? 112 : gcn[3]};
      light_uv(be16(clipped), clamp);
      assert(memcmp(stock, chosen, sizeof(stock)) == 0);
      lane_bad += memcmp(stock, lane, sizeof(stock)) != 0;
      swap_bad += memcmp(stock, swap, sizeof(stock)) != 0;
      clamp_bad += memcmp(stock, clamp, sizeof(stock)) != 0;
   }
   assert(lane_bad == 65535 && swap_bad == 65040 && clamp_bad == 7455);
   assert(!exact_half_position(0.1f, &half_z));
   assert(!exact_half_position(0.000001f, &half_z));
   printf("all 65536 scaled light values exact, bad lane %u, swapped bytes %u, bad clamp %u\n",
          lane_bad, swap_bad, clamp_bad);
   return 0;
}
