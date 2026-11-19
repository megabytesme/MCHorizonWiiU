#pragma once
#include "hud.h"

typedef struct HorizonHudGpuVertex {
   float position[4], color[4], uv[2], glint_uv[2];
} HorizonHudGpuVertex;

int horizon_hud_rgba_bytes(uint32_t width, uint32_t height, size_t *bytes);
HorizonHudResult horizon_hud_validate(const HorizonHudFrame *frame, uint32_t atlas_width,
                                       uint32_t atlas_height, uint32_t capacity);
void horizon_hud_encode_vertices(const HorizonHudFrame *frame, HorizonHudGpuVertex *output);
int horizon_hud_depth_scissor(const HorizonHudFrame *frame, const HorizonHudRun *run,
                              uint32_t x, uint32_t y, uint32_t width, uint32_t height,
                              uint32_t output[4]);
