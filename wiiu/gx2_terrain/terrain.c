#include "terrain.h"
#include <stdlib.h>
#include <string.h>
#include <coreinit/debug.h>
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

#include "terrain_buffers.h"

_Static_assert(sizeof(HorizonTerrainVertex) == 40, "terrain vertex ABI changed");
struct HorizonGx2Terrain {
   WHBGfxShaderGroup shader;
   GX2Texture texture;
   GX2Sampler sampler;
   uint32_t sampler_slot;
   uint32_t clip_offsets[4];
   uint32_t section_origin_offset;
   uint32_t fog_color_offset;
   uint32_t fog_ranges_offset;
   GX2RBuffer quad_indices;
   uint32_t quad_index_allocation_bytes;
   int shader_loaded;
};
static const GX2RResourceFlags texture_flags = GX2R_RESOURCE_BIND_TEXTURE |
   GX2R_RESOURCE_USAGE_CPU_WRITE | GX2R_RESOURCE_USAGE_GPU_READ;

static int uniform_offset(const GX2UniformVar *var, uint32_t *offset)
{
   if (!var || var->type != GX2_SHADER_VAR_TYPE_FLOAT4 || var->count != 1 ||
       var->block >= 0) return 0;
   *offset = var->offset;
   return 1;
}

void horizon_gx2_terrain_destroy(HorizonGx2Terrain *terrain)
{
   if (!terrain) return;
   GX2DrawDone();
   terrain_buffer_release(&terrain->quad_indices);
   if (terrain->texture.surface.image) GX2RDestroySurfaceEx(&terrain->texture.surface, texture_flags);
   if (terrain->shader_loaded) WHBGfxFreeShaderGroup(&terrain->shader);
   free(terrain);
}
uint32_t horizon_gx2_terrain_index_allocation_bytes(const HorizonGx2Terrain *terrain)
{
   return terrain ? terrain->quad_index_allocation_bytes : 0;
}
int horizon_gx2_terrain_create(HorizonGx2Terrain **out, const void *gfd,
                               uint32_t width, uint32_t height,
                               const uint8_t *rgba, size_t length)
{
   if (!out) return -1;
   *out = NULL;
   if (!gfd || !rgba || !width || !height || width > 8192 || height > 8192 ||
       (size_t)width > SIZE_MAX / height / 4 || length < (size_t)width * height * 4) return -1;
   HorizonGx2Terrain *t = calloc(1, sizeof(*t));
   if (!t) return -2;
   int result = -2;
   if (!WHBGfxLoadGFDShaderGroup(&t->shader, 0, gfd)) goto fail;
   t->shader_loaded = 1;
   if (!t->shader.vertexShader || !t->shader.pixelShader ||
       t->shader.vertexShader->mode != GX2_SHADER_MODE_UNIFORM_REGISTER ||
       t->shader.pixelShader->mode != GX2_SHADER_MODE_UNIFORM_REGISTER) {
      result = -3; goto fail;
   }
   GX2PixelShader *ps = t->shader.pixelShader;
   int found = 0;
   if (ps && ps->samplerVars) for (uint32_t i = 0; i < ps->samplerVarCount; ++i) {
      const GX2SamplerVar *v = &ps->samplerVars[i];
      if (v->name && !strcmp(v->name, "uTerrain") && v->type == GX2_SAMPLER_VAR_TYPE_SAMPLER_2D) {
         t->sampler_slot = v->location; found = 1; break;
      }
   }
   if (!found) { result = -3; goto fail; }
   static const char *clip_names[4] = { "uClipX", "uClipY", "uClipZ", "uClipW" };
   GX2VertexShader *vs = t->shader.vertexShader;
   for (uint32_t row = 0; row < 4; ++row)
      if (!uniform_offset(GX2GetVertexUniformVar(vs, clip_names[row]), &t->clip_offsets[row])) {
         result = -3; goto fail;
      }
   if (!uniform_offset(GX2GetVertexUniformVar(vs, "uSectionOrigin"), &t->section_origin_offset) ||
       !uniform_offset(GX2GetPixelUniformVar(ps, "uFogColor"), &t->fog_color_offset) ||
       !uniform_offset(GX2GetPixelUniformVar(ps, "uFogRanges"), &t->fog_ranges_offset)) {
      result = -3; goto fail;
   }
   if (!WHBGfxInitShaderAttribute(&t->shader, "aPosition", 0, offsetof(HorizonTerrainVertex, position), GX2_ATTRIB_FORMAT_FLOAT_32_32_32_32) ||
       !WHBGfxInitShaderAttribute(&t->shader, "aColor", 0, offsetof(HorizonTerrainVertex, color), GX2_ATTRIB_FORMAT_FLOAT_32_32_32) ||
       !WHBGfxInitShaderAttribute(&t->shader, "aUv", 0, offsetof(HorizonTerrainVertex, uv), GX2_ATTRIB_FORMAT_FLOAT_32_32) ||
       !WHBGfxInitShaderAttribute(&t->shader, "aCutout", 0, offsetof(HorizonTerrainVertex, cutout), GX2_ATTRIB_FORMAT_FLOAT_32) ||
       !WHBGfxInitFetchShader(&t->shader)) goto fail;
   GX2Surface *s = &t->texture.surface;
   s->use = GX2_SURFACE_USE_TEXTURE; s->dim = GX2_SURFACE_DIM_TEXTURE_2D;
   s->width = width; s->height = height; s->depth = 1; s->mipLevels = 1;
   s->format = GX2_SURFACE_FORMAT_UNORM_R8_G8_B8_A8; s->aa = GX2_AA_MODE1X;
   s->tileMode = GX2_TILE_MODE_LINEAR_ALIGNED;
   t->texture.viewNumMips = 1; t->texture.viewNumSlices = 1;
   t->texture.compMap = GX2_COMP_MAP(GX2_SQ_SEL_R, GX2_SQ_SEL_G, GX2_SQ_SEL_B, GX2_SQ_SEL_A);
   if (!GX2RCreateSurface(s, texture_flags)) goto fail;
   if (s->pitch < width || (size_t)s->imageSize < (size_t)s->pitch * height * 4) {
      result = -3; goto fail;
   }
   uint8_t *pixels = GX2RLockSurfaceEx(s, 0, texture_flags);
   if (!pixels) goto fail;
   for (uint32_t y = 0; y < height; ++y)
      memcpy(pixels + (size_t)y * s->pitch * 4, rgba + (size_t)y * width * 4, (size_t)width * 4);
   GX2RUnlockSurfaceEx(s, 0, texture_flags);
   GX2InitTextureRegs(&t->texture);
   GX2InitSampler(&t->sampler, GX2_TEX_CLAMP_MODE_CLAMP, GX2_TEX_XY_FILTER_MODE_POINT);
   result = terrain_index_buffer_create(&t->quad_indices, &t->quad_index_allocation_bytes);
   if (result) goto fail;
   OSReport("Horizon memory gx2_terrain texture_bytes=%u\n", s->imageSize);
   *out = t; return 0;
fail:
   horizon_gx2_terrain_destroy(t); return result;
}
void horizon_gx2_terrain_wait_idle(void) { GX2DrawDone(); }
int horizon_gx2_terrain_draw(HorizonGx2Terrain *t, HorizonGx2TerrainSection *s,
                            uint32_t target_width, uint32_t target_height,
                            uint32_t x, uint32_t y, uint32_t width, uint32_t height,
                            const float clip_rows[16], const float section_origin[4],
                            const float fog_color[4], const float fog_ranges[4])
{
   if (!t || !s || !target_width || !target_height || !width || !height ||
       x > target_width || y > target_height || width > target_width-x || height > target_height-y ||
       !clip_rows || !section_origin || !fog_color || !fog_ranges) return -1;
   if (!s->vertex_count) return 0;
   GX2SetShaderMode(t->shader.vertexShader->mode);
   GX2SetVertexShader(t->shader.vertexShader); GX2SetPixelShader(t->shader.pixelShader);
   for (uint32_t row = 0; row < 4; ++row)
      GX2SetVertexUniformReg(t->clip_offsets[row], 4, &clip_rows[row * 4]);
   GX2SetVertexUniformReg(t->section_origin_offset, 4, section_origin);
   GX2SetPixelUniformReg(t->fog_color_offset, 4, fog_color);
   GX2SetPixelUniformReg(t->fog_ranges_offset, 4, fog_ranges);
   GX2SetViewport(x, y, width, height, 0, 1); GX2SetScissor(x, y, width, height);
   GX2SetFetchShader(&t->shader.fetchShader);
   GX2RSetAttributeBuffer(&s->buffer, 0, sizeof(HorizonTerrainVertex), offsetof(HorizonTerrainVertex, position));
   GX2RSetAttributeBuffer(&s->buffer, 1, sizeof(HorizonTerrainVertex), offsetof(HorizonTerrainVertex, color));
   GX2RSetAttributeBuffer(&s->buffer, 2, sizeof(HorizonTerrainVertex), offsetof(HorizonTerrainVertex, uv));
   GX2RSetAttributeBuffer(&s->buffer, 3, sizeof(HorizonTerrainVertex), offsetof(HorizonTerrainVertex, cutout));
   GX2SetPixelTexture(&t->texture, t->sampler_slot); GX2SetPixelSampler(&t->sampler, t->sampler_slot);
   GX2SetDepthOnlyControl(TRUE, TRUE, GX2_COMPARE_FUNC_LEQUAL);
   GX2SetBlendControl(GX2_RENDER_TARGET_0, GX2_BLEND_MODE_ONE, GX2_BLEND_MODE_ZERO,
                      GX2_BLEND_COMBINE_MODE_ADD, TRUE, GX2_BLEND_MODE_ONE,
                      GX2_BLEND_MODE_ZERO, GX2_BLEND_COMBINE_MODE_ADD);
   GX2SetAlphaTest(FALSE, GX2_COMPARE_FUNC_ALWAYS, 0.0f);
   GX2SetCullOnlyControl(GX2_FRONT_FACE_CCW, FALSE, FALSE);
   GX2SetRasterizerClipControlEx(TRUE, TRUE, TRUE);
   GX2SetColorControl(GX2_LOGIC_OP_COPY, 0, FALSE, TRUE);
   GX2SetTargetChannelMasks(GX2_CHANNEL_MASK_RGBA, (GX2ChannelMask)0, (GX2ChannelMask)0,
                            (GX2ChannelMask)0, (GX2ChannelMask)0, (GX2ChannelMask)0, (GX2ChannelMask)0, (GX2ChannelMask)0);
   terrain_draw_indexed(&t->quad_indices, s->vertex_count);
   return 0;
}
