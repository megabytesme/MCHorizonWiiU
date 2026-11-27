#include "light-encoding.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>

int main(void)
{
   unsigned swapped = 0, wrong_format = 0;
   for (unsigned light = 0; light <= UINT16_MAX; ++light) {
      const float packed = (float)light;
      const float high = floorf(packed * (1.0f / 256.0f));
      const float low = packed - high * 256.0f;
      const float stock[2] = {
         fminf(high, 240.0f) * (1.0f / 256.0f) + 0.5f / 16.0f,
         fminf(low, 240.0f) * (1.0f / 256.0f) + 0.5f / 16.0f,
      };
      const float encoded = centered_light((uint16_t)light);
      const float whole = floorf(encoded);
      const float decoded[2] = {whole * (1.0f / 256.0f), encoded - whole};
      const float reverse[2] = {decoded[1], decoded[0]};
      const float wrong[2] = {packed * (1.0f / 256.0f), 0};
      assert(memcmp(stock, decoded, sizeof(stock)) == 0);
      swapped += memcmp(stock, reverse, sizeof(stock)) != 0;
      wrong_format += memcmp(stock, wrong, sizeof(stock)) != 0;
   }
   assert(swapped == 65040 && wrong_format == 65536);
   printf("centered-light oracle matched %u pairs, swapped=%u wrong-format=%u\n",
          UINT16_MAX + 1u, swapped, wrong_format);
   return 0;
}
