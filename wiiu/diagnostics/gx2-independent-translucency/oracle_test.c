#include "scene.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>

static void oracle(unsigned w, unsigned h, unsigned camera, unsigned lane, uint8_t out[2][4])
{
   float rows[16];
   fixture_project(camera,(float)w/h,rows);
   double color[3] = {0,0,0}, last_depth = 1;
   const double opacity = 128.0/255.0;
   for (unsigned first = 0; first < VIEWS; ++first) {
      // Each slab has both windings. Stock culling leaves one face to blend.
      const unsigned red = lane == 0 ? first : 1-first;
      const double z = red ? 1 : -1;
      const double clip_w = rows[14]*z + rows[15];
      const double depth = (rows[10]*z + rows[11])/clip_w;
      assert(0 < depth && depth < 1 && (clip_w == 5 || clip_w == 7));
      const double x_extent = fabs(2*rows[0]/clip_w);
      const double y_extent = fabs(2*rows[5]/clip_w);
      const double offset_x = 2.0*(h/15+0.5)/w, offset_y = 2.0*(h/15+0.5)/h;
      assert(offset_x < x_extent && offset_y < y_extent);
      assert(1-2.0*(w/16+0.5)/w > x_extent);
      if (depth <= last_depth) {
         for (unsigned channel = 0; channel < 3; ++channel)
            color[channel] = (channel == (red ? 0u : 2u) ? 255*opacity : 0) +
                 color[channel]*(1-opacity);
         last_depth = depth;
      }
   }
   for (unsigned i = 0; i < 3; ++i) {
      out[0][i] = (uint8_t)floor(color[i]);
      out[1][i] = (uint8_t)round(color[i]);
   }
   out[0][3] = out[1][3] = 255;
}

int main(void)
{
   const unsigned size[][2] = {{1920,1080},{854,480}};
   for (enum Mode mode=INDEPENDENT; mode<=SWAP_LANES; ++mode) {
      unsigned observed = 0;
      for (unsigned output=0; output<VIEWS; ++output) {
         const unsigned camera = view_camera(mode,output), lane = view_lane(mode,output);
         uint8_t pixels[2][4];
         oracle(size[output][0],size[output][1],camera,lane,pixels);
         const bool correct = !(fault_mask(mode)&(1u<<output));
         if (!matches_pixel(pixels[1],camera,true)) observed |= 1u<<output;
         for (unsigned rounding=0; rounding<2; ++rounding)
            assert(matches_pixel(pixels[rounding],camera,correct));
         assert(!matches_pixel(pixels[1],camera^1u,true));
      }
      assert(observed == fault_mask(mode));
   }
   puts("GX2 independent blend oracle PASS");
   return 0;
}
