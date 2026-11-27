#include "scene.h"
#include <assert.h>
#include <stdio.h>

typedef struct { double x, y, depth; } Pixel;

static Pixel project(Rect r, const float rows[16], double x, double y, double z)
{
   double clip[4];
   for (unsigned i=0; i<4; ++i)
      clip[i] = x*rows[i*4] + y*rows[i*4+1] + z*rows[i*4+2] + rows[i*4+3];
   assert(clip[3] > 0);
   return (Pixel){r.x + (clip[0]/clip[3]+1)*r.w*0.5,
                  r.y + (1-clip[1]/clip[3])*r.h*0.5, clip[2]/clip[3]};
}

static bool in_box(Pixel lower_left, Pixel upper_right, double x, double y)
{
   return lower_left.x < x && x < upper_right.x && upper_right.y < y && y < lower_left.y;
}

static const uint8_t *model(Rect r, unsigned seat, unsigned pose, bool distant,
                            enum Fault fault, Sample sample)
{
   const unsigned site_index = distant ? seat : 0;
   const unsigned copy = fault == WRONG_SITE && seat == SEATS-1 ? 0 : site_index;
   const unsigned camera = fault == REUSE_CAMERA ? 0 : seat;
   const double x = sample.x + 0.5, y = sample.y + 0.5;
   if (x > r.x+marker_x(seat) && x < r.x+marker_x(seat)+9 &&
       y > r.y+marker_y(pose) && y < r.y+marker_y(pose)+9)
      return palette[SEATS-1-seat];
   if (y < r.y || y > r.y+r.h ||
       (fault != NO_SCISSOR && (x < r.x+GAP || x > r.x+r.w-GAP)))
      return clear;

   float rows[16], origin[4];
   section_projection(r, site_index, camera, pose, 0, rows, origin);
   assert(origin[0] == -eye_x(camera, pose) && origin[1] == -8 && origin[2] == -12);
   const Pixel a = project(r, rows, -64, -32, 0), b = project(r, rows, 64, 48, 0);
   const uint8_t *winner = clear;
   double depth = 1;
   if (in_box(a,b,x,y)) {
      winner = row_color(seat);
      depth = a.depth;
   }
   for (unsigned i=0; i<SEATS; ++i) {
      const double mid = 3.0 + 2.5*i;
      const Pixel p = project(r, rows, mid-0.5, 6.8, 4), q = project(r, rows, mid+0.5, 9.2, 4);
      assert(0 < p.depth && p.depth < a.depth && a.depth < 1);
      if (fault != NO_DEPTH && p.depth < depth && in_box(p,q,x,y)) {
         winner = stripe_color(i, copy);
         depth = p.depth;
      }
   }
   return winner;
}

static void exercise(uint32_t w, uint32_t h, unsigned views)
{
   unsigned tv_pixels = 0;
   Rect rects[4];
   for (unsigned i=0; i<views; ++i) {
      const Rect r = rects[i] = viewport(w,h,views,0,i);
      assert(r.x+r.w <= w && r.y+r.h <= h);
      for (unsigned j=0; j<i; ++j) {
         const Rect p = rects[j];
         assert(r.x+r.w <= p.x || p.x+p.w <= r.x || r.y+r.h <= p.y || p.y+p.h <= r.y);
      }
      tv_pixels += r.w*r.h;
   }
   assert(tv_pixels == w*h);
   if (views == 4) {
      for (unsigned column = 0; column < 2; ++column) {
         const Rect upper = rects[column], lower = rects[column+2];
         const Sample boundary = {upper.x+upper.w/2,upper.y+upper.h-2,row_color(column)};
         const Rect leak = world_viewport(lower, column+2, CROSS_VIEW);
         assert(!memcmp(model(upper,column,0,false,GOOD,boundary), row_color(column), 4));
         assert(!memcmp(model(lower,column+2,0,false,GOOD,boundary),clear,4));
         assert(!memcmp(model(leak,column+2,0,false,CROSS_VIEW,boundary),row_color(column+2),4));
      }
   }

   for (unsigned i=0; i<=views; ++i) {
      const unsigned seat = i == views ? SEATS-1 : i;
      const Rect r = i == views ? viewport(854,480,views,1,seat) : rects[i];
      for (unsigned d=0; d<=1; ++d)
         for (unsigned pose=0; pose<=1; ++pose)
            for (enum Fault fault=GOOD; fault<=WRONG_SITE; ++fault) {
               if (pose && fault > GENERAL_SHADER) continue;
               if (!d && fault == WRONG_SITE) continue;
               Sample expected[CHECKS];
               samples(r, seat, pose, d, expected);
               unsigned wrong = 0;
               for (unsigned i=0; i<CHECKS; ++i)
                  if (memcmp(model(r,seat,pose,d,fault,expected[i]),expected[i].color,4))
                     wrong |= 1u << i;
               assert(wrong == expected_mask(fault,seat));
            }
   }
}

int main(void)
{
   const unsigned dims[][2] = {{1920,1080},{1280,720},{1281,721}};
   const unsigned views[] = {1,2,4};
   for (unsigned d=0; d<3; ++d)
      for (unsigned l=0; l<3; ++l)
         exercise(dims[d][0],dims[d][1],views[l]);
   // Synthetic site four contains a unique white stripe. TV site geometry and
   // HUD have no white pixel from which a final-color crop could recover it.
   for (unsigned i=0; i<SEATS-1; ++i)
      for (unsigned j=0; j<SEATS; ++j)
         assert(memcmp(stripe_color(j,i),drc_witness,4) && memcmp(palette[j],drc_witness,4));
   assert(!memcmp(stripe_color(SEATS-1,SEATS-1),drc_witness,4));
   puts("GX2 independent views analytic oracle PASS");
   return 0;
}
