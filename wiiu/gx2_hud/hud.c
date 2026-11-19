#include "hud_data.h"
#include "cit_glint.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <coreinit/debug.h>
#include <gx2/clear.h>
#include <gx2/context.h>
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
_Static_assert(sizeof(HorizonHudRun) == 28 && _Alignof(HorizonHudRun) == 4, "HUD run ABI changed");
_Static_assert(offsetof(HorizonHudRun, start) == 0, "HUD run start offset changed");
_Static_assert(offsetof(HorizonHudRun, end) == 4, "HUD run end offset changed");
_Static_assert(offsetof(HorizonHudRun, blend) == 8, "HUD run blend offset changed");
_Static_assert(offsetof(HorizonHudRun, clip) == 12, "HUD run clip offset changed");
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
   GX2Texture atlas, glint;
   HorizonHudCit *cit;
   GX2Sampler sampler, linear_sampler, glint_sampler;
   uint32_t width, height, capacity, atlas_slot, glint_slot, pass_offset, glint_offset;
   uint32_t portal_offsets[3];
   uint32_t cit_tint_offset;
   float glint_style[4];
   float portal[3][4];
   int portal_ready;
   int shader_loaded;
};

static const GX2RResourceFlags texture_flags = GX2R_RESOURCE_BIND_TEXTURE |
   GX2R_RESOURCE_USAGE_CPU_WRITE | GX2R_RESOURCE_USAGE_GPU_READ;

void horizon_hud_texture_destroy(GX2Texture *texture)
{
   if (texture->surface.image) GX2RDestroySurfaceEx(&texture->surface, texture_flags);
}

void horizon_gx2_hud_destroy(HorizonGx2Hud *hud)
{
   if (!hud) return;
   GX2DrawDone();
   if (GX2RBufferExists(&hud->buffer)) GX2RDestroyBufferEx(&hud->buffer, 0);
   horizon_hud_cit_destroy(hud->cit);
   horizon_hud_texture_destroy(&hud->glint);
   horizon_hud_texture_destroy(&hud->atlas);
   if (hud->shader_loaded) WHBGfxFreeShaderGroup(&hud->shader);
   free(hud);
}

int horizon_hud_texture_create(GX2Texture *texture, uint32_t width, uint32_t height, const uint8_t *rgba)
{
   GX2Surface *surface = &texture->surface;
   surface->use = GX2_SURFACE_USE_TEXTURE;
   surface->dim = GX2_SURFACE_DIM_TEXTURE_2D;
   surface->width = width;
   surface->height = height;
   surface->depth = 1;
   surface->mipLevels = 1;
   surface->format = GX2_SURFACE_FORMAT_UNORM_R8_G8_B8_A8;
   surface->aa = GX2_AA_MODE1X;
   surface->tileMode = GX2_TILE_MODE_LINEAR_ALIGNED;
   texture->viewNumMips = 1;
   texture->viewNumSlices = 1;
   texture->compMap = GX2_COMP_MAP(GX2_SQ_SEL_R, GX2_SQ_SEL_G, GX2_SQ_SEL_B, GX2_SQ_SEL_A);
   if (!GX2RCreateSurface(surface, texture_flags)) return 0;
   uint8_t *pixels = GX2RLockSurfaceEx(surface, 0, texture_flags);
   if (!pixels) {
      GX2RDestroySurfaceEx(surface, texture_flags);
      memset(texture, 0, sizeof(*texture));
      return 0;
   }
   for (uint32_t y = 0; y < height; ++y)
      memcpy(pixels + (size_t)y * surface->pitch * 4, rgba + (size_t)y * width * 4, (size_t)width * 4);
   GX2RUnlockSurfaceEx(surface, 0, texture_flags);
   GX2InitTextureRegs(texture);
   return 1;
}

HorizonHudResult horizon_gx2_hud_set_glint_texture(HorizonGx2Hud *hud, uint32_t width, uint32_t height,
                                                  const uint8_t *rgba, size_t length)
{
   size_t bytes;
   if (!hud || hud->glint.surface.image || !width || !height || width > 16384 || height > 16384 ||
       !rgba || !horizon_hud_rgba_bytes(width, height, &bytes) || bytes > length)
      return HORIZON_HUD_INVALID;
   return horizon_hud_texture_create(&hud->glint, width, height, rgba)
      ? HORIZON_HUD_OK : HORIZON_HUD_ALLOCATION_FAILED;
}

void horizon_gx2_hud_set_glint_style(HorizonGx2Hud *hud, float strength, const float tint[3])
{
   if (!hud) return;
   hud->glint_style[0] = strength;
   for (uint32_t channel = 0; channel < 3; ++channel)
      hud->glint_style[channel + 1] = tint ? tint[channel] : -1.0f;
}

void horizon_gx2_hud_set_end_portal(HorizonGx2Hud *hud, const float sprites[2][4], float time)
{
   if (!hud || !sprites) return;
   memcpy(hud->portal, sprites, 2 * sizeof(hud->portal[0]));
   hud->portal[2][0] = time;
   hud->portal_ready = 1;
}

HorizonHudResult horizon_gx2_hud_set_cit_glints(HorizonGx2Hud *hud, uint32_t width, uint32_t height,
                                                const uint8_t *pixels, size_t length,
                                                const HorizonHudCitSlot *slots, uint32_t count)
{
   if (!hud || hud->cit) return HORIZON_HUD_INVALID;
   return horizon_hud_cit_create(&hud->cit, width, height, pixels, length, slots, count);
}

static int pixel_uniform_offset(const GX2PixelShader *shader, const char *name, uint32_t *offset)
{
   const GX2UniformVar *v = GX2GetPixelUniformVar(shader, name);
   if (!v || v->type != GX2_SHADER_VAR_TYPE_FLOAT4 || v->count != 1 || v->block >= 0) return 0;
   *offset = v->offset;
   return 1;
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
   if (!pixel_shader || pixel_shader->mode != GX2_SHADER_MODE_UNIFORM_REGISTER ||
       hud->shader.vertexShader->mode != GX2_SHADER_MODE_UNIFORM_REGISTER) {
      failure = HORIZON_HUD_UNSUPPORTED; goto fail;
   }
   int found_atlas = 0, found_glint = 0;
   if (pixel_shader && pixel_shader->samplerVars) {
      for (uint32_t i = 0; i < pixel_shader->samplerVarCount; ++i) {
         const GX2SamplerVar *sampler = &pixel_shader->samplerVars[i];
         if (!sampler->name || sampler->type != GX2_SAMPLER_VAR_TYPE_SAMPLER_2D) continue;
         if (strcmp(sampler->name, "uAtlas") == 0) {
            hud->atlas_slot = sampler->location;
            found_atlas = 1;
         } else if (strcmp(sampler->name, "uGlint") == 0) {
            hud->glint_slot = sampler->location;
            found_glint = 1;
         }
      }
   }
   if (!found_atlas || !found_glint ||
       !pixel_uniform_offset(pixel_shader, "uPass", &hud->pass_offset) ||
       !pixel_uniform_offset(pixel_shader, "uGlintStyle", &hud->glint_offset) ||
       !pixel_uniform_offset(pixel_shader, "uCitTint", &hud->cit_tint_offset) ||
       !pixel_uniform_offset(pixel_shader, "uEndSky", &hud->portal_offsets[0]) ||
       !pixel_uniform_offset(pixel_shader, "uEndPortal", &hud->portal_offsets[1]) ||
       !pixel_uniform_offset(pixel_shader, "uPortalTime", &hud->portal_offsets[2])) {
      failure = HORIZON_HUD_UNSUPPORTED; goto fail;
   }
   if (!WHBGfxInitShaderAttribute(&hud->shader, "aPosition", 0, offsetof(HorizonHudGpuVertex, position), GX2_ATTRIB_FORMAT_FLOAT_32_32_32_32) ||
       !WHBGfxInitShaderAttribute(&hud->shader, "aColor", 0, offsetof(HorizonHudGpuVertex, color), GX2_ATTRIB_FORMAT_FLOAT_32_32_32_32) ||
       !WHBGfxInitShaderAttribute(&hud->shader, "aUv", 0, offsetof(HorizonHudGpuVertex, uv), GX2_ATTRIB_FORMAT_FLOAT_32_32) ||
       !WHBGfxInitShaderAttribute(&hud->shader, "aGlintUv", 0, offsetof(HorizonHudGpuVertex, glint_uv), GX2_ATTRIB_FORMAT_FLOAT_32_32) ||
       !WHBGfxInitFetchShader(&hud->shader)) goto fail;
   hud->buffer.flags = GX2R_RESOURCE_BIND_VERTEX_BUFFER | GX2R_RESOURCE_USAGE_CPU_WRITE | GX2R_RESOURCE_USAGE_GPU_READ;
   hud->buffer.elemSize = sizeof(HorizonHudGpuVertex);
   hud->buffer.elemCount = capacity;
   if (!GX2RCreateBuffer(&hud->buffer)) goto fail;
   if (!horizon_hud_texture_create(&hud->atlas, width, height, rgba)) goto fail;
   GX2InitSampler(&hud->sampler, GX2_TEX_CLAMP_MODE_CLAMP, GX2_TEX_XY_FILTER_MODE_POINT);
   GX2InitSampler(&hud->linear_sampler, GX2_TEX_CLAMP_MODE_CLAMP, GX2_TEX_XY_FILTER_MODE_LINEAR);
   GX2InitSampler(&hud->glint_sampler, GX2_TEX_CLAMP_MODE_WRAP, GX2_TEX_XY_FILTER_MODE_LINEAR);
   horizon_gx2_hud_set_glint_style(hud, 0.75f, NULL);
   hud->width = width; hud->height = height; hud->capacity = capacity;
   OSReport("Horizon memory gx2_hud atlas_bytes=%u vertex_payload_bytes=%u\n",
            hud->atlas.surface.imageSize, hud->buffer.elemSize * hud->buffer.elemCount);
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

static void bind_glint(HorizonGx2Hud *hud)
{
   GX2SetPixelTexture(hud->glint.surface.image ? &hud->glint : &hud->atlas, hud->glint_slot);
   GX2SetPixelSampler(&hud->glint_sampler, hud->glint_slot);
}

static void set_hud_state(HorizonGx2Hud *hud, uint32_t x, uint32_t y,
                          uint32_t width, uint32_t height)
{
   GX2SetViewport((float)x, (float)y, (float)width, (float)height, 0, 1);
   GX2SetScissor(x, y, width, height);
   GX2SetShaderMode(hud->shader.pixelShader->mode);
   GX2SetVertexShader(hud->shader.vertexShader);
   GX2SetPixelShader(hud->shader.pixelShader);
   GX2SetFetchShader(&hud->shader.fetchShader);
   GX2RSetAttributeBuffer(&hud->buffer, 0, sizeof(HorizonHudGpuVertex), 0);
   GX2SetPixelTexture(&hud->atlas, hud->atlas_slot);
   bind_glint(hud);
   GX2SetPixelUniformReg(hud->glint_offset, 4, hud->glint_style);
   GX2SetDepthOnlyControl(FALSE, FALSE, GX2_COMPARE_FUNC_ALWAYS);
   GX2SetAlphaTest(FALSE, GX2_COMPARE_FUNC_ALWAYS, 0.0f);
   GX2SetCullOnlyControl(GX2_FRONT_FACE_CCW, FALSE, FALSE);
   GX2SetRasterizerClipControlEx(TRUE, TRUE, TRUE);
   GX2SetColorControl(GX2_LOGIC_OP_COPY, 1, FALSE, TRUE);
   GX2SetTargetChannelMasks(GX2_CHANNEL_MASK_RGBA, (GX2ChannelMask)0, (GX2ChannelMask)0,
                            (GX2ChannelMask)0, (GX2ChannelMask)0, (GX2ChannelMask)0,
                            (GX2ChannelMask)0, (GX2ChannelMask)0);
}

typedef struct HudOutput {
   GX2ContextState *context;
   GX2ColorBuffer *color;
   GX2DepthBuffer *depth;
} HudOutput;

static HorizonHudResult draw_to(HorizonGx2Hud *hud, const HorizonHudFrame *f,
                                uint32_t x, uint32_t y, uint32_t width, uint32_t height,
                                const HudOutput *output)
{
   if (!hud || !f) return HORIZON_HUD_INVALID;
   HorizonHudResult validation = horizon_hud_validate(f, hud->width, hud->height, hud->capacity);
   if (validation != HORIZON_HUD_OK) return validation;
   validation = validate_target_viewport(f, x, y, width, height);
   if (validation != HORIZON_HUD_OK) return validation;
   if (!hud->glint.surface.image)
      for (uint32_t i = 0; i < f->vertex_count; ++i)
         if (f->vertices[i].uv[0] >= 2.0f) return HORIZON_HUD_UNSUPPORTED;
   for (uint32_t i = 0; i < f->run_count; ++i) {
      const uint32_t blend = f->runs[i].blend;
      if (blend == HORIZON_HUD_END_PORTAL && !hud->portal_ready) return HORIZON_HUD_UNSUPPORTED;
      if (blend == HORIZON_HUD_CIT_GLINT) {
         validation = horizon_hud_cit_validate(hud->cit, f, &f->runs[i]);
         if (validation != HORIZON_HUD_OK) return validation;
      }
      if (blend == HORIZON_HUD_DEPTH &&
          (!output || !output->context || !output->color || !output->color->surface.image ||
           !output->depth || !output->depth->surface.image))
         return HORIZON_HUD_UNSUPPORTED;
   }
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
   set_hud_state(hud, x, y, width, height);
   int depth_active = 0;
   for (uint32_t i = 0; i < f->run_count; ++i) {
      const uint32_t blend = f->runs[i].blend;
      if (blend == HORIZON_HUD_DEPTH) {
         uint32_t clip[4];
         if (!horizon_hud_depth_scissor(f, &f->runs[i], x, y, width, height, clip)) continue;
         GX2ClearDepthStencilEx(output->depth, 1.0f, 0, GX2_CLEAR_FLAGS_DEPTH);
         /* Like WHBGfxClearColor, restore the output context after a clear.
          * Rebind attachments before replaying this pass's draw state. */
         GX2SetContextState(output->context);
         GX2SetColorBuffer(output->color, GX2_RENDER_TARGET_0);
         GX2SetDepthBuffer(output->depth);
         set_hud_state(hud, x, y, width, height);
         GX2SetDepthOnlyControl(TRUE, TRUE, GX2_COMPARE_FUNC_LEQUAL);
         GX2SetScissor(clip[0], clip[1], clip[2], clip[3]);
         depth_active = 1;
      } else if (depth_active) {
         GX2SetDepthOnlyControl(FALSE, FALSE, GX2_COMPARE_FUNC_ALWAYS);
         GX2SetScissor(x, y, width, height);
         depth_active = 0;
      }
      const int vignette = blend == HORIZON_HUD_VIGNETTE;
      const float pass[4] = {(float)blend};
      GX2SetPixelUniformReg(hud->pass_offset, 4, pass);
      GX2SetPixelSampler(vignette ? &hud->linear_sampler : &hud->sampler, hud->atlas_slot);
      if (blend == HORIZON_HUD_CIT_GLINT) {
         horizon_hud_cit_draw(hud->cit, f, &f->runs[i], hud->glint_slot, hud->cit_tint_offset);
         bind_glint(hud);
         continue;
      }
      if (blend == HORIZON_HUD_END_PORTAL) {
         for (uint32_t j = 0; j < 3; ++j)
            GX2SetPixelUniformReg(hud->portal_offsets[j], 4, hud->portal[j]);
         GX2SetBlendControl(GX2_RENDER_TARGET_0, GX2_BLEND_MODE_ONE, GX2_BLEND_MODE_ZERO,
                            GX2_BLEND_COMBINE_MODE_ADD, FALSE, GX2_BLEND_MODE_ONE,
                            GX2_BLEND_MODE_ZERO, GX2_BLEND_COMBINE_MODE_ADD);
      } else if (blend == HORIZON_HUD_ADDITIVE) {
         /* Horizon's BlendFunction.ADDITIVE: ONE, ONE for color and alpha. */
         GX2SetBlendControl(GX2_RENDER_TARGET_0, GX2_BLEND_MODE_ONE, GX2_BLEND_MODE_ONE,
                            GX2_BLEND_COMBINE_MODE_ADD, TRUE, GX2_BLEND_MODE_ONE,
                            GX2_BLEND_MODE_ONE, GX2_BLEND_COMBINE_MODE_ADD);
      } else if (blend == HORIZON_HUD_INVERT) {
         /* Match Horizon's INVERT function: (1 - dst) * src + (1 - src) * dst. */
         GX2SetBlendControl(GX2_RENDER_TARGET_0, GX2_BLEND_MODE_INV_DST_COLOR,
                            GX2_BLEND_MODE_INV_SRC_COLOR, GX2_BLEND_COMBINE_MODE_ADD,
                            TRUE, GX2_BLEND_MODE_ONE, GX2_BLEND_MODE_ZERO,
                            GX2_BLEND_COMBINE_MODE_ADD);
      } else if (vignette) {
         GX2SetBlendControl(GX2_RENDER_TARGET_0, GX2_BLEND_MODE_ZERO,
                            GX2_BLEND_MODE_INV_SRC_COLOR, GX2_BLEND_COMBINE_MODE_ADD,
                            TRUE, GX2_BLEND_MODE_ZERO, GX2_BLEND_MODE_ONE,
                            GX2_BLEND_COMBINE_MODE_ADD);
      } else {
         GX2SetBlendControl(GX2_RENDER_TARGET_0, GX2_BLEND_MODE_SRC_ALPHA, GX2_BLEND_MODE_INV_SRC_ALPHA,
                            GX2_BLEND_COMBINE_MODE_ADD, TRUE, GX2_BLEND_MODE_ONE,
                            GX2_BLEND_MODE_INV_SRC_ALPHA, GX2_BLEND_COMBINE_MODE_ADD);
      }
      GX2DrawEx(GX2_PRIMITIVE_MODE_TRIANGLES, f->runs[i].end - f->runs[i].start, f->runs[i].start, 1);
   }
   if (depth_active) {
      GX2SetDepthOnlyControl(FALSE, FALSE, GX2_COMPARE_FUNC_ALWAYS);
      GX2SetScissor(x, y, width, height);
   }
   return HORIZON_HUD_OK;
}

HorizonHudResult horizon_gx2_hud_draw_to(HorizonGx2Hud *hud, const HorizonHudFrame *f,
                                         uint32_t x, uint32_t y,
                                         uint32_t width, uint32_t height)
{
   return draw_to(hud, f, x, y, width, height, NULL);
}

HorizonHudResult horizon_gx2_hud_draw_to_output(HorizonGx2Hud *hud, const HorizonHudFrame *f,
                                                uint32_t x, uint32_t y,
                                                uint32_t width, uint32_t height,
                                                uint32_t output)
{
   if (output > 1) return HORIZON_HUD_INVALID;
   const HudOutput target = output == 0
      ? (HudOutput){WHBGfxGetTVContextState(), WHBGfxGetTVColourBuffer(), WHBGfxGetTVDepthBuffer()}
      : (HudOutput){WHBGfxGetDRCContextState(), WHBGfxGetDRCColourBuffer(), WHBGfxGetDRCDepthBuffer()};
   return draw_to(hud, f, x, y, width, height, &target);
}

HorizonHudResult horizon_gx2_hud_draw(HorizonGx2Hud *hud, const HorizonHudFrame *f)
{
   if (!hud || !f) return HORIZON_HUD_INVALID;
   return horizon_gx2_hud_draw_to(hud, f, 0, 0, f->viewport_width, f->viewport_height);
}
