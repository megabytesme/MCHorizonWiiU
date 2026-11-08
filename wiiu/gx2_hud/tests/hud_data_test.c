#include "../hud_data.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>

static int near(float a, float b) { return fabsf(a - b) < 0.00001f; }

int main(void)
{
   size_t bytes;
   assert(horizon_hud_rgba_bytes(8, 8, &bytes) && bytes == 256);
   assert(!horizon_hud_rgba_bytes(UINT32_MAX, UINT32_MAX, &bytes));
#if SIZE_MAX == UINT32_MAX
   assert(!horizon_hud_rgba_bytes(65536, 16384, &bytes));
#endif
   HorizonHudVertex vertices[6] = {0};
   vertices[0].pos[0] = 213.5f; vertices[0].pos[1] = 120;
   vertices[1].pos[0] = 427; vertices[1].pos[1] = 240;
   vertices[0].color = UINT32_C(0x80402010);
   vertices[0].uv[0] = 0.25f; vertices[0].uv[1] = 0.75f;
   HorizonHudRun runs[] = {{0, 3, HORIZON_HUD_ALPHA}, {3, 6, HORIZON_HUD_ALPHA}};
   uint8_t rgba[16] = {0};
   HorizonHudPatch patch = {2, 3, 2, 2, rgba, sizeof(rgba)};
   /* DRC is the source layout. draw_to may render this same NDC geometry in
    * another same-aspect target rectangle without changing these dimensions. */
   HorizonHudFrame frame = {vertices, 6, runs, 2, &patch, 1, 854, 480, 2.0f, 0};
   assert(horizon_hud_validate(&frame, 8, 8, 6) == HORIZON_HUD_OK);
   HorizonHudGpuVertex out[6];
   horizon_hud_encode_vertices(&frame, out);
   assert(near(out[0].position[0], 0.0f) && near(out[0].position[1], 0.0f));
   assert(near(out[1].position[0], 1.0f) && near(out[1].position[1], -1.0f));
   assert(near(out[0].color[0], 16.0f / 255.0f));
   assert(near(out[0].color[1], 32.0f / 255.0f));
   assert(near(out[0].color[2], 64.0f / 255.0f));
   assert(near(out[0].color[3], 128.0f / 255.0f));
   assert(near(out[0].uv[0], 0.25f) && near(out[0].uv[1], 0.75f));
   runs[1].start = 0;
   assert(horizon_hud_validate(&frame, 8, 8, 6) == HORIZON_HUD_INVALID);
   runs[1].start = 3; runs[1].blend = HORIZON_HUD_INVERT;
   assert(horizon_hud_validate(&frame, 8, 8, 6) == HORIZON_HUD_UNSUPPORTED);
   runs[1].blend = HORIZON_HUD_ADDITIVE;
   assert(horizon_hud_validate(&frame, 8, 8, 6) == HORIZON_HUD_OK);
   runs[1].blend = HORIZON_HUD_ALPHA; patch.x = 7;
   assert(horizon_hud_validate(&frame, 8, 8, 6) == HORIZON_HUD_INVALID);
   patch.x = 2; patch.length = 15;
   assert(horizon_hud_validate(&frame, 8, 8, 6) == HORIZON_HUD_INVALID);
   patch.length = sizeof(rgba); patch.width = 0;
   assert(horizon_hud_validate(&frame, 8, 8, 6) == HORIZON_HUD_INVALID);
   patch.width = 2; patch.height = 0;
   assert(horizon_hud_validate(&frame, 8, 8, 6) == HORIZON_HUD_INVALID);
   patch.height = 2; vertices[0].uv[0] = 2.0f;
   assert(horizon_hud_validate(&frame, 8, 8, 6) == HORIZON_HUD_UNSUPPORTED);
   vertices[0].uv[0] = 0.25f; frame.pack_text_shader = 1;
   assert(horizon_hud_validate(&frame, 8, 8, 6) == HORIZON_HUD_UNSUPPORTED);
   frame.pack_text_shader = 0;
   HorizonHudPatch overflow = {0, 0, UINT32_MAX, UINT32_MAX, rgba, SIZE_MAX};
   frame.patches = &overflow;
   assert(horizon_hud_validate(&frame, UINT32_MAX, UINT32_MAX, 6) == HORIZON_HUD_INVALID);
   puts("HUD data validation passed");
   return 0;
}
