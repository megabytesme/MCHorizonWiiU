#pragma once
#include <stddef.h>
#include <stdint.h>

/* Layout of Horizon's repr(C) HudVertex on the Cafe target. */
typedef struct HorizonHudVertex {
   float pos[2], uv[2];
   uint32_t color;
   float depth, glint_uv[2];
   uint32_t text;
} HorizonHudVertex;

typedef struct HorizonHudPatch {
   uint32_t x, y, width, height;
   const uint8_t *rgba;
   size_t length;
} HorizonHudPatch;

typedef enum HorizonHudBlend {
   HORIZON_HUD_ALPHA = 0,
   HORIZON_HUD_INVERT = 1,
   HORIZON_HUD_ADDITIVE = 2,
   HORIZON_HUD_DEPTH = 3,
   HORIZON_HUD_VIGNETTE = 4,
   HORIZON_HUD_END_PORTAL = 5,
   HORIZON_HUD_CIT_GLINT = 6,
} HorizonHudBlend;

typedef struct HorizonHudRun {
   uint32_t start, end, blend;
} HorizonHudRun;

typedef struct HorizonHudFrame {
   const HorizonHudVertex *vertices;
   uint32_t vertex_count;
   const HorizonHudRun *runs;
   uint32_t run_count;
   const HorizonHudPatch *patches;
   uint32_t patch_count;
   uint32_t viewport_width, viewport_height;
   float scale;
   /* Nonzero when a pack-provided GUI text shader must be used. */
   uint32_t pack_text_shader;
} HorizonHudFrame;

typedef enum HorizonHudResult {
   HORIZON_HUD_OK = 0,
   HORIZON_HUD_INVALID = -1,
   HORIZON_HUD_UNSUPPORTED = -2,
   HORIZON_HUD_ALLOCATION_FAILED = -3,
} HorizonHudResult;

typedef struct HorizonGx2Hud HorizonGx2Hud;

/* The GFD shader bytes must remain alive until destroy; atlas is copied. */
HorizonHudResult horizon_gx2_hud_create(HorizonGx2Hud **out, const void *gfd,
                                        uint32_t atlas_width, uint32_t atlas_height,
                                        const uint8_t *rgba, size_t rgba_length,
                                        uint32_t vertex_capacity);
/* Draw into the caller's active GX2 color target; this changes GX2 render state. */
HorizonHudResult horizon_gx2_hud_draw(HorizonGx2Hud *hud, const HorizonHudFrame *frame);
/* Draw the source-layout frame into a target viewport rectangle. The source
 * viewport and scale still normalize vertices; target dimensions only select
 * the GX2 viewport/scissor. The target must preserve source aspect (within
 * 0.5%); the caller may letterbox it. */
HorizonHudResult horizon_gx2_hud_draw_to(HorizonGx2Hud *hud, const HorizonHudFrame *frame,
                                         uint32_t x, uint32_t y,
                                         uint32_t width, uint32_t height);
void horizon_gx2_hud_destroy(HorizonGx2Hud *hud);
