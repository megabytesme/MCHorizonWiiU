#include "hud_data.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <gx2/draw.h>
#include <gx2/event.h>
#include <gx2/registers.h>
#include <gx2/sampler.h>
#include <gx2/shaders.h>
#include <gx2/texture.h>
#include <gx2/utils.h>
#include <gx2r/buffer.h>
#include <gx2r/draw.h>
#include <gx2r/surface.h>
#include <whb/gfx.h>

_Static_assert(sizeof(HorizonHudVertex) == 36, "Horizon HudVertex layout changed");
_Static_assert(_Alignof(HorizonHudVertex) == 4, "Horizon HudVertex alignment changed");
_Static_assert(offsetof(HorizonHudVertex, uv) == 8, "Horizon UV offset changed");
_Static_assert(offsetof(HorizonHudVertex, color) == 16, "Horizon color offset changed");
_Static_assert(offsetof(HorizonHudVertex, depth) == 20, "Horizon depth offset changed");
_Static_assert(offsetof(HorizonHudVertex, glint_uv) == 24, "Horizon glint UV offset changed");
_Static_assert(offsetof(HorizonHudVertex, text) == 32, "Horizon text offset changed");

/* The Rust Cafe bridge declares the same 32-bit C ABI in wiiu/platform/src/hud.rs. */
_Static_assert(sizeof(HorizonHudRun) == 12 && _Alignof(HorizonHudRun) == 4, "HUD run ABI changed");
_Static_assert(offsetof(HorizonHudRun, start) == 0, "HUD run start offset changed");
_Static_assert(offsetof(HorizonHudRun, end) == 4, "HUD run end offset changed");
_Static_assert(offsetof(HorizonHudRun, blend) == 8, "HUD run blend offset changed");
_Static_assert(sizeof(HorizonHudPatch) == 24 && _Alignof(HorizonHudPatch) == 4, "HUD patch ABI changed");
_Static_assert(offsetof(HorizonHudPatch, x) == 0, "HUD patch x offset changed");
_Static_assert(offsetof(HorizonHudPatch, y) == 4, "HUD patch y offset changed");
_Static_assert(offsetof(HorizonHudPatch, width) == 8, "HUD patch width offset changed");
_Static_assert(offsetof(HorizonHudPatch, height) == 12, "HUD patch height offset changed");
_Static_assert(offsetof(HorizonHudPatch, rgba) == 16, "HUD patch pixels offset changed");
_Static_assert(offsetof(HorizonHudPatch, length) == 20, "HUD patch length offset changed");
_Static_assert(sizeof(HorizonHudFrame) == 40 && _Alignof(HorizonHudFrame) == 4, "HUD frame ABI changed");
_Static_assert(offsetof(HorizonHudFrame, vertices) == 0, "HUD frame vertices offset changed");
_Static_assert(offsetof(HorizonHudFrame, vertex_count) == 4, "HUD frame vertex count offset changed");
_Static_assert(offsetof(HorizonHudFrame, runs) == 8, "HUD frame runs offset changed");
_Static_assert(offsetof(HorizonHudFrame, run_count) == 12, "HUD frame run count offset changed");
_Static_assert(offsetof(HorizonHudFrame, patches) == 16, "HUD frame patches offset changed");
_Static_assert(offsetof(HorizonHudFrame, patch_count) == 20, "HUD frame patch count offset changed");
_Static_assert(offsetof(HorizonHudFrame, viewport_width) == 24, "HUD frame viewport width offset changed");
_Static_assert(offsetof(HorizonHudFrame, viewport_height) == 28, "HUD frame viewport height offset changed");
_Static_assert(offsetof(HorizonHudFrame, scale) == 32, "HUD frame scale offset changed");
_Static_assert(offsetof(HorizonHudFrame, pack_text_shader) == 36, "HUD frame shader offset changed");

struct HorizonGx2Hud {
   WHBGfxShaderGroup shader;
   GX2RBuffer buffer;
   GX2Texture atlas;
   GX2Sampler sampler;
   uint32_t width, height, capacity, atlas_slot;
   int shader_loaded;
};

static const GX2RResourceFlags texture_flags = GX2R_RESOURCE_BIND_TEXTURE |
   GX2R_RESOURCE_USAGE_CPU_WRITE | GX2R_RESOURCE_USAGE_GPU_READ;

void horizon_gx2_hud_destroy(HorizonGx2Hud *hud)
{
   if (!hud) return;
   GX2DrawDone();
   if (GX2RBufferExists(&hud->buffer)) GX2RDestroyBufferEx(&hud->buffer, 0);
   if (hud->atlas.surface.image) GX2RDestroySurfaceEx(&hud->atlas.surface, texture_flags);
   if (hud->shader_loaded) WHBGfxFreeShaderGroup(&hud->shader);
   free(hud);
}

HorizonHudResult horizon_gx2_hud_create(HorizonGx2Hud **out, const void *gfd,
                                        uint32_t width, uint32_t height,
                                        const uint8_t *rgba, size_t length,
                                        uint32_t capacity)
{
   if (!out) return HORIZON_HUD_INVALID;
   *out = NULL;
   size_t atlas_bytes;
   if (!gfd || !rgba || !width || !height || !capacity ||
       width > 16384 || height > 16384 || !horizon_hud_rgba_bytes(width, height, &atlas_bytes) ||
       atlas_bytes > length ||
       capacity > UINT32_MAX / sizeof(HorizonHudGpuVertex)) return HORIZON_HUD_INVALID;
   HorizonGx2Hud *hud = calloc(1, sizeof(*hud));
   if (!hud) return HORIZON_HUD_ALLOCATION_FAILED;
   HorizonHudResult failure = HORIZON_HUD_ALLOCATION_FAILED;
   if (!WHBGfxLoadGFDShaderGroup(&hud->shader, 0, gfd)) goto fail;
   hud->shader_loaded = 1;
   GX2PixelShader *pixel_shader = hud->shader.pixelShader;
   int found_atlas = 0;
   if (pixel_shader && pixel_shader->samplerVars) {
      for (uint32_t i = 0; i < pixel_shader->samplerVarCount; ++i) {
         const GX2SamplerVar *sampler = &pixel_shader->samplerVars[i];
         if (sampler->name && strcmp(sampler->name, "uAtlas") == 0 &&
             sampler->type == GX2_SAMPLER_VAR_TYPE_SAMPLER_2D) {
            hud->atlas_slot = sampler->location;
            found_atlas = 1;
            break;
         }
      }
   }
   if (!found_atlas) { failure = HORIZON_HUD_UNSUPPORTED; goto fail; }
   if (!WHBGfxInitShaderAttribute(&hud->shader, "aPosition", 0, offsetof(HorizonHudGpuVertex, position), GX2_ATTRIB_FORMAT_FLOAT_32_32_32_32) ||
       !WHBGfxInitShaderAttribute(&hud->shader, "aColor", 0, offsetof(HorizonHudGpuVertex, color), GX2_ATTRIB_FORMAT_FLOAT_32_32_32_32) ||
       !WHBGfxInitShaderAttribute(&hud->shader, "aUv", 0, offsetof(HorizonHudGpuVertex, uv), GX2_ATTRIB_FORMAT_FLOAT_32_32) ||
       !WHBGfxInitFetchShader(&hud->shader)) goto fail;
   hud->buffer.flags = GX2R_RESOURCE_BIND_VERTEX_BUFFER | GX2R_RESOURCE_USAGE_CPU_WRITE | GX2R_RESOURCE_USAGE_GPU_READ;
   hud->buffer.elemSize = sizeof(HorizonHudGpuVertex);
   hud->buffer.elemCount = capacity;
   if (!GX2RCreateBuffer(&hud->buffer)) goto fail;
   GX2Surface *surface = &hud->atlas.surface;
   surface->use = GX2_SURFACE_USE_TEXTURE;
   surface->dim = GX2_SURFACE_DIM_TEXTURE_2D;
   surface->width = width;
   surface->height = height;
   surface->depth = 1;
   surface->mipLevels = 1;
   surface->format = GX2_SURFACE_FORMAT_UNORM_R8_G8_B8_A8;
   surface->aa = GX2_AA_MODE1X;
   surface->tileMode = GX2_TILE_MODE_LINEAR_ALIGNED;
   hud->atlas.viewNumMips = 1;
   hud->atlas.viewNumSlices = 1;
   hud->atlas.compMap = GX2_COMP_MAP(GX2_SQ_SEL_R, GX2_SQ_SEL_G, GX2_SQ_SEL_B, GX2_SQ_SEL_A);
   if (!GX2RCreateSurface(surface, texture_flags)) goto fail;
   uint8_t *pixels = GX2RLockSurfaceEx(surface, 0, texture_flags);
   if (!pixels) goto fail;
   for (uint32_t y = 0; y < height; ++y)
      memcpy(pixels + (size_t)y * surface->pitch * 4, rgba + (size_t)y * width * 4, (size_t)width * 4);
   GX2RUnlockSurfaceEx(surface, 0, texture_flags);
   GX2InitTextureRegs(&hud->atlas);
   GX2InitSampler(&hud->sampler, GX2_TEX_CLAMP_MODE_CLAMP, GX2_TEX_XY_FILTER_MODE_POINT);
   hud->width = width; hud->height = height; hud->capacity = capacity;
   *out = hud;
   return HORIZON_HUD_OK;
fail:
   horizon_gx2_hud_destroy(hud);
   return failure;
}

static HorizonHudResult validate_target_viewport(const HorizonHudFrame *f,
                                                  uint32_t x, uint32_t y,
                                                  uint32_t width, uint32_t height)
{
   if (!f || !f->viewport_width || !f->viewport_height || !width || !height ||
       x > UINT32_MAX - width || y > UINT32_MAX - height)
      return HORIZON_HUD_INVALID;

   /* Let the host letterbox mismatched outputs instead of stretching the UI.
    * The small tolerance accounts for the DRC's 854x480 pixel dimensions. */
   uint64_t target_scaled = (uint64_t)width * f->viewport_height;
   uint64_t source_scaled = (uint64_t)height * f->viewport_width;
   uint64_t difference = target_scaled > source_scaled
      ? target_scaled - source_scaled : source_scaled - target_scaled;
   uint64_t largest = target_scaled > source_scaled ? target_scaled : source_scaled;
   if (difference > largest / 200)
      return HORIZON_HUD_UNSUPPORTED;

   return HORIZON_HUD_OK;
}

HorizonHudResult horizon_gx2_hud_draw_to(HorizonGx2Hud *hud, const HorizonHudFrame *f,
                                         uint32_t x, uint32_t y,
                                         uint32_t width, uint32_t height)
{
   if (!hud || !f) return HORIZON_HUD_INVALID;
   HorizonHudResult validation = horizon_hud_validate(f, hud->width, hud->height, hud->capacity);
   if (validation != HORIZON_HUD_OK) return validation;
   validation = validate_target_viewport(f, x, y, width, height);
   if (validation != HORIZON_HUD_OK) return validation;
   /* The previous frame may still read these resources. */
   GX2DrawDone();
   if (f->patch_count) {
      uint8_t *pixels = GX2RLockSurfaceEx(&hud->atlas.surface, 0, texture_flags);
      if (!pixels) return HORIZON_HUD_ALLOCATION_FAILED;
      for (uint32_t i = 0; i < f->patch_count; ++i) {
         const HorizonHudPatch *p = &f->patches[i];
         for (uint32_t y = 0; y < p->height; ++y)
            memcpy(pixels + ((size_t)(p->y + y) * hud->atlas.surface.pitch + p->x) * 4,
                   p->rgba + (size_t)y * p->width * 4, (size_t)p->width * 4);
      }
      GX2RUnlockSurfaceEx(&hud->atlas.surface, 0, texture_flags);
   }
   if (!f->vertex_count) return HORIZON_HUD_OK;
   HorizonHudGpuVertex *out = GX2RLockBufferEx(&hud->buffer, 0);
   if (!out) return HORIZON_HUD_ALLOCATION_FAILED;
   horizon_hud_encode_vertices(f, out);
   GX2RUnlockBufferEx(&hud->buffer, 0);
   GX2SetViewport((float)x, (float)y, (float)width, (float)height, 0, 1);
   GX2SetScissor(x, y, width, height);
   GX2SetVertexShader(hud->shader.vertexShader);
   GX2SetPixelShader(hud->shader.pixelShader);
   GX2SetFetchShader(&hud->shader.fetchShader);
   GX2RSetAttributeBuffer(&hud->buffer, 0, sizeof(HorizonHudGpuVertex), 0);
   GX2SetPixelTexture(&hud->atlas, hud->atlas_slot);
   GX2SetPixelSampler(&hud->sampler, hud->atlas_slot);
   GX2SetDepthOnlyControl(FALSE, FALSE, GX2_COMPARE_FUNC_ALWAYS);
   GX2SetAlphaTest(FALSE, GX2_COMPARE_FUNC_ALWAYS, 0.0f);
   GX2SetCullOnlyControl(GX2_FRONT_FACE_CCW, FALSE, FALSE);
   GX2SetRasterizerClipControlEx(TRUE, TRUE, TRUE);
   GX2SetColorControl(GX2_LOGIC_OP_COPY, 1, FALSE, TRUE);
   GX2SetTargetChannelMasks(GX2_CHANNEL_MASK_RGBA, (GX2ChannelMask)0, (GX2ChannelMask)0,
                            (GX2ChannelMask)0, (GX2ChannelMask)0, (GX2ChannelMask)0,
                            (GX2ChannelMask)0, (GX2ChannelMask)0);
   for (uint32_t i = 0; i < f->run_count; ++i) {
      if (f->runs[i].blend == HORIZON_HUD_ADDITIVE) {
         /* Horizon's BlendFunction.ADDITIVE: ONE, ONE for color and alpha. */
         GX2SetBlendControl(GX2_RENDER_TARGET_0, GX2_BLEND_MODE_ONE, GX2_BLEND_MODE_ONE,
                            GX2_BLEND_COMBINE_MODE_ADD, TRUE, GX2_BLEND_MODE_ONE,
                            GX2_BLEND_MODE_ONE, GX2_BLEND_COMBINE_MODE_ADD);
      } else {
         GX2SetBlendControl(GX2_RENDER_TARGET_0, GX2_BLEND_MODE_SRC_ALPHA, GX2_BLEND_MODE_INV_SRC_ALPHA,
                            GX2_BLEND_COMBINE_MODE_ADD, TRUE, GX2_BLEND_MODE_ONE,
                            GX2_BLEND_MODE_INV_SRC_ALPHA, GX2_BLEND_COMBINE_MODE_ADD);
      }
      GX2DrawEx(GX2_PRIMITIVE_MODE_TRIANGLES, f->runs[i].end - f->runs[i].start, f->runs[i].start, 1);
   }
   return HORIZON_HUD_OK;
}

HorizonHudResult horizon_gx2_hud_draw(HorizonGx2Hud *hud, const HorizonHudFrame *f)
{
   if (!hud || !f) return HORIZON_HUD_INVALID;
   return horizon_gx2_hud_draw_to(hud, f, 0, 0, f->viewport_width, f->viewport_height);
}
