#pragma once

#include <math.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

enum { SEATS = 5, SECTIONS = 174, TILE_SIDE = 20, GAP = 4 };
enum Fault { GOOD, GENERAL_SHADER, REUSE_CAMERA, NO_DEPTH, NO_SCISSOR, WRONG_SITE, CROSS_VIEW };
enum Check { CENTER, STRIPE, EDGE, HUD, SEAM, CHECKS };
typedef struct { uint32_t x, y, w, h; } Rect;
typedef struct { uint32_t x, y; const uint8_t *color; } Sample;

static const uint8_t palette[SEATS][4] = {
   {255, 0, 0, 255}, {0, 255, 0, 255}, {0, 0, 255, 255}, {255, 255, 0, 255}, {255, 0, 255, 255},
};
static const uint8_t clear[4] = {0, 0, 0, 255};
static const uint8_t backdrop[4] = {0, 255, 255, 255};
static const uint8_t drc_witness[4] = {255, 255, 255, 255};

void fixture_project(double x, double y, double z, float aspect, float out[16]);

static inline Rect viewport(uint32_t w, uint32_t h, unsigned tv_views, unsigned output, unsigned id)
{
   if (output || tv_views == 1) return (Rect){0, 0, w, h};
   const unsigned left = w / 2, top = tv_views == 4 ? h / 2 : h;
   const bool right = id % 2, bottom = id >= 2;
   return (Rect){right ? left : 0, bottom ? top : 0, right ? w-left : left, bottom ? h-top : top};
}

static inline const uint8_t *row_color(unsigned seat)
{
   static const uint8_t lower[4] = {32, 64, 128, 255};
   return seat == 2 || seat == 3 ? lower : backdrop;
}

static inline Rect world_viewport(Rect panel, unsigned seat, enum Fault fault)
{
   if (fault == CROSS_VIEW && (seat == 2 || seat == 3)) {
      // Deliberately cover the neighboring upper panel by 16 rows.
      panel.y -= 16;
      panel.h += 16;
   }
   return panel;
}

static inline unsigned site(unsigned seat, bool distant) { return distant ? seat : 0; }
static inline double stripe_x(unsigned seat) { return 3.0 + 2.5 * seat; }
static inline double eye_x(unsigned seat, unsigned pose) { return stripe_x(seat) + 1.125 * pose; }
static inline double site_x(unsigned index) { return 1024.0 * index; }
static inline const uint8_t *stripe_color(unsigned seat, unsigned index)
{
   return seat == SEATS-1 && index == SEATS-1 ? drc_witness : palette[(seat + index) % SEATS];
}

static inline void section_projection(Rect r, unsigned index, unsigned seat, unsigned pose,
                                      unsigned section, float rows[16], float origin[4])
{
   // Distinct section addresses share one nearby site; distant sites are 1024 blocks apart.
   const int column = section ? (int)((section - 1) % 13) - 6 : 0;
   const unsigned row = section ? (section - 1) / 13 : 0;
   const double x = site_x(index) + 16.0 * column;
   const double y = section ? -22.0 : 0.0;
   const double z = section ? -24.0 - 16.0 * row : 0.0;
   const double rel_x = x - (site_x(index) + eye_x(seat, pose));
   const double rel_y = y - 8.0;
   const double rel_z = z - 12.0;
   fixture_project(rel_x, rel_y, rel_z, (float)r.w / r.h, rows);
   origin[0] = (float)rel_x;
   origin[1] = (float)rel_y;
   origin[2] = (float)rel_z;
   origin[3] = 0.0f;
}

static inline unsigned marker_x(unsigned seat) { return 12 + 11 * seat; }
static inline unsigned marker_y(unsigned pose) { return 13 + 12 * pose; }

static inline void samples(Rect r, unsigned seat, unsigned pose, bool distant, Sample out[CHECKS])
{
   // With focal length 1, (world x - eye x) / distance * viewport height / 2
   // is the horizontal pixel offset from the view center. The stripe is 8 blocks away.
   const double projected_stripe = r.w * 0.5 + (stripe_x(seat) + 0.30 - eye_x(seat, pose)) * r.h / 16.0;
   const uint8_t *stripe = stripe_color(seat, site(seat, distant));
   out[CENTER] = (Sample){r.x + r.w/2, r.y + r.h/2, pose ? row_color(seat) : stripe};
   out[STRIPE] = (Sample){r.x + (uint32_t)projected_stripe, r.y + r.h/2, stripe};
   out[EDGE] = (Sample){r.x + 1, r.y + r.h/2, clear};
   out[HUD] = (Sample){r.x + marker_x(seat) + 4, r.y + marker_y(pose) + 4, palette[SEATS-1-seat]};
   out[SEAM] = (Sample){r.x + r.w/2, r.y + 1, row_color(seat)};
}

static inline unsigned expected_mask(enum Fault fault, unsigned seat)
{
   const unsigned world = (1u << CENTER) | (1u << STRIPE);
   if (fault == REUSE_CAMERA && seat != 0) return world;
   if (fault == NO_DEPTH) return world;
   if (fault == NO_SCISSOR) return 1u << EDGE;
   if (fault == WRONG_SITE && seat == SEATS-1) return world;
   if (fault == CROSS_VIEW && seat < 2) return 1u << SEAM;
   return 0;
}
