#include "terrain.h"
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

_Static_assert(sizeof(HorizonTerrainVertex) == 40, "terrain vertex ABI changed");
struct HorizonGx2Terrain {
   WHBGfxShaderGroup shader;
   GX2Texture texture;
   GX2Sampler sampler;
   uint32_t sampler_slot;
   int shader_loaded;
};
struct HorizonGx2TerrainSection { GX2RBuffer buffer; uint32_t count; };
static const GX2RResourceFlags texture_flags = GX2R_RESOURCE_BIND_TEXTURE |
   GX2R_RESOURCE_USAGE_CPU_WRITE | GX2R_RESOURCE_USAGE_GPU_READ;

void horizon_gx2_terrain_section_destroy(HorizonGx2TerrainSection *section)
{
   if (!section) return;
   GX2DrawDone();
   if (GX2RBufferExists(&section->buffer)) GX2RDestroyBufferEx(&section->buffer, 0);
   free(section);
}
void horizon_gx2_terrain_destroy(HorizonGx2Terrain *terrain)
{
   if (!terrain) return;
   GX2DrawDone();
   if (terrain->texture.surface.image) GX2RDestroySurfaceEx(&terrain->texture.surface, texture_flags);
   if (terrain->shader_loaded) WHBGfxFreeShaderGroup(&terrain->shader);
   free(terrain);
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
   GX2PixelShader *ps = t->shader.pixelShader;
   int found = 0;
   if (ps && ps->samplerVars) for (uint32_t i = 0; i < ps->samplerVarCount; ++i) {
      const GX2SamplerVar *v = &ps->samplerVars[i];
      if (v->name && !strcmp(v->name, "uTerrain") && v->type == GX2_SAMPLER_VAR_TYPE_SAMPLER_2D) {
         t->sampler_slot = v->location; found = 1; break;
      }
   }
   if (!found) { result = -3; goto fail; }
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
   *out = t; return 0;
fail:
   horizon_gx2_terrain_destroy(t); return result;
}
int horizon_gx2_terrain_section_create(HorizonGx2TerrainSection **out,
                                      const HorizonTerrainVertex *vertices, uint32_t count)
{
   if (!out || (count && !vertices) || count % 3) return -1;
   *out = NULL;
   HorizonGx2TerrainSection *s = calloc(1, sizeof(*s));
   if (!s) return -2;
   s->count = count;
   if (count) {
      s->buffer.flags = GX2R_RESOURCE_BIND_VERTEX_BUFFER | GX2R_RESOURCE_USAGE_CPU_WRITE | GX2R_RESOURCE_USAGE_GPU_READ;
      s->buffer.elemSize = sizeof(HorizonTerrainVertex); s->buffer.elemCount = count;
      if (!GX2RCreateBuffer(&s->buffer)) { free(s); return -2; }
      void *dst = GX2RLockBufferEx(&s->buffer, 0);
      if (!dst) { horizon_gx2_terrain_section_destroy(s); return -2; }
      memcpy(dst, vertices, (size_t)count * sizeof(*vertices)); GX2RUnlockBufferEx(&s->buffer, 0);
   }
   *out = s; return 0;
}
int horizon_gx2_terrain_section_update(HorizonGx2TerrainSection *s,
                                      const HorizonTerrainVertex *vertices, uint32_t count)
{
   if (!s || count != s->count || (count && !vertices)) return -1;
   if (!count) return 0;
   void *dst = GX2RLockBufferEx(&s->buffer, 0);
   if (!dst) return -2;
   memcpy(dst, vertices, (size_t)count * sizeof(*vertices)); GX2RUnlockBufferEx(&s->buffer, 0);
   return 0;
}
void horizon_gx2_terrain_wait_idle(void) { GX2DrawDone(); }
int horizon_gx2_terrain_draw(HorizonGx2Terrain *t, HorizonGx2TerrainSection *s,
                            uint32_t target_width, uint32_t target_height,
                            uint32_t x, uint32_t y, uint32_t width, uint32_t height)
{
   if (!t || !s || !target_width || !target_height || !width || !height ||
       x > target_width || y > target_height || width > target_width-x || height > target_height-y) return -1;
   if (!s->count) return 0;
   GX2SetViewport(x, y, width, height, 0, 1); GX2SetScissor(x, y, width, height);
   GX2SetVertexShader(t->shader.vertexShader); GX2SetPixelShader(t->shader.pixelShader);
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
   GX2DrawEx(GX2_PRIMITIVE_MODE_TRIANGLES, s->count, 0, 1);
   return 0;
}
