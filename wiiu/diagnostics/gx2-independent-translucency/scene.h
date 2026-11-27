#pragma once
#include <stdbool.h>
#include <stdint.h>

enum { VIEWS = 2, QUADS = 4, INDICES = QUADS * 6 };
enum Mode { INDEPENDENT, SWAP_CAMERAS, REUSE_TV_LANE, SWAP_LANES };

void fixture_project(uint32_t camera, float aspect, float rows[16]);

static inline unsigned view_camera(enum Mode mode, unsigned output)
{
   return mode == SWAP_CAMERAS ? output ^ 1u : output;
}

static inline unsigned view_lane(enum Mode mode, unsigned output)
{
   if (mode == REUSE_TV_LANE) return 0;
   if (mode == SWAP_LANES) return output ^ 1u;
   return view_camera(mode, output);
}

// Reference: the U8-normalized source alpha is 128/255 over opaque black.
// The nearer contribution is exactly 128. The far contribution is
// 128*(127/255) = 63.749..., hence only 63 or 64 on UNORM conversion.
static inline bool matches_pixel(const uint8_t rgba[4], unsigned camera, bool far_visible)
{
   const unsigned near = camera ? 2 : 0, far = near ^ 2u;
   return rgba[near] == 128 && rgba[1] == 0 && rgba[3] == 255 &&
      (far_visible ? rgba[far] == 63 || rgba[far] == 64 : rgba[far] == 0);
}

static inline unsigned fault_mask(enum Mode mode)
{
   if (mode == REUSE_TV_LANE) return 2;
   if (mode == SWAP_LANES) return 3;
   return 0;
}
