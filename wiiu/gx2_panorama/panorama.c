#include "panorama.h"
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

_Static_assert(sizeof(HorizonPanoramaVertex) == 28, "panorama vertex ABI changed");
_Static_assert(offsetof(HorizonPanoramaVertex, direction) == 16, "panorama direction offset changed");

struct HorizonGx2Panorama {
   WHBGfxShaderGroup shader;
   GX2RBuffer buffer;
   GX2Texture cube;
   GX2Sampler sampler;
   uint32_t slot;
   int shader_loaded;
};

static const GX2RResourceFlags texture_flags = GX2R_RESOURCE_BIND_TEXTURE |
   GX2R_RESOURCE_USAGE_CPU_WRITE | GX2R_RESOURCE_USAGE_GPU_READ;

void horizon_gx2_panorama_destroy(HorizonGx2Panorama *p)
{
   if (!p) return;
   GX2DrawDone();
   if (GX2RBufferExists(&p->buffer)) GX2RDestroyBufferEx(&p->buffer, 0);
   if (p->cube.surface.image) GX2RDestroySurfaceEx(&p->cube.surface, texture_flags);
   if (p->shader_loaded) WHBGfxFreeShaderGroup(&p->shader);
   free(p);
}

int horizon_gx2_panorama_create(HorizonGx2Panorama **out, const void *gfd,
                                uint32_t size, const uint8_t *rgba, size_t length)
{
   if (!out) return -1;
   *out = NULL;
   if (!gfd || !rgba || !size || size > 4096 ||
       (size_t)size > SIZE_MAX / size / 24 || length < (size_t)size * size * 24) return -1;
   HorizonGx2Panorama *p = calloc(1, sizeof(*p));
   if (!p) return -2;
   int result = -2;
   if (!WHBGfxLoadGFDShaderGroup(&p->shader, 0, gfd)) goto fail;
   p->shader_loaded = 1;
   int found = 0;
   GX2PixelShader *shader = p->shader.pixelShader;
   if (shader && shader->samplerVars) {
      for (uint32_t i = 0; i < shader->samplerVarCount; ++i) {
         const GX2SamplerVar *v = &shader->samplerVars[i];
         if (v->name && !strcmp(v->name, "uPanorama") && v->type == GX2_SAMPLER_VAR_TYPE_SAMPLER_2D) {
            p->slot = v->location;
            found = 1;
            break;
         }
      }
   }
   if (!found) { result = -3; goto fail; }
   if (!WHBGfxInitShaderAttribute(&p->shader, "aPosition", 0, offsetof(HorizonPanoramaVertex, position), GX2_ATTRIB_FORMAT_FLOAT_32_32_32_32) ||
       !WHBGfxInitShaderAttribute(&p->shader, "aDirection", 0, offsetof(HorizonPanoramaVertex, direction), GX2_ATTRIB_FORMAT_FLOAT_32_32_32) ||
       !WHBGfxInitFetchShader(&p->shader)) goto fail;
   p->buffer.flags = GX2R_RESOURCE_BIND_VERTEX_BUFFER | GX2R_RESOURCE_USAGE_CPU_WRITE | GX2R_RESOURCE_USAGE_GPU_READ;
   p->buffer.elemSize = sizeof(HorizonPanoramaVertex);
   p->buffer.elemCount = 36;
   if (!GX2RCreateBuffer(&p->buffer)) goto fail;
   GX2Surface *s = &p->cube.surface;
   s->use = GX2_SURFACE_USE_TEXTURE;
   s->dim = GX2_SURFACE_DIM_TEXTURE_2D;
   s->width = size; s->height = size * 6; s->depth = 1; s->mipLevels = 1;
   s->format = GX2_SURFACE_FORMAT_UNORM_R8_G8_B8_A8;
   s->aa = GX2_AA_MODE1X;
   s->tileMode = GX2_TILE_MODE_LINEAR_ALIGNED;
   p->cube.viewNumMips = 1; p->cube.viewNumSlices = 1;
   p->cube.compMap = GX2_COMP_MAP(GX2_SQ_SEL_R, GX2_SQ_SEL_G, GX2_SQ_SEL_B, GX2_SQ_SEL_A);
   if (!GX2RCreateSurface(s, texture_flags)) goto fail;
   if ((size_t)s->imageSize < (size_t)s->pitch * size * 6 * 4) {
      result = -3;
      goto fail;
   }
   uint8_t *pixels = GX2RLockSurfaceEx(s, 0, texture_flags);
   if (!pixels) goto fail;
   const size_t slice = (size_t)s->pitch * size * 4;
   for (uint32_t face = 0; face < 6; ++face)
      for (uint32_t y = 0; y < size; ++y)
         memcpy(pixels + face * slice + (size_t)y * s->pitch * 4,
                rgba + ((size_t)face * size + y) * size * 4, (size_t)size * 4);
   GX2RUnlockSurfaceEx(s, 0, texture_flags);
   GX2InitTextureRegs(&p->cube);
   GX2InitSampler(&p->sampler, GX2_TEX_CLAMP_MODE_CLAMP, GX2_TEX_XY_FILTER_MODE_LINEAR);
   *out = p;
   return 0;
fail:
   horizon_gx2_panorama_destroy(p);
   return result;
}

int horizon_gx2_panorama_draw(HorizonGx2Panorama *p, uint32_t width, uint32_t height,
                              const HorizonPanoramaVertex vertices[36])
{
   if (!p || !width || !height || !vertices) return -1;
   GX2DrawDone();
   void *buffer = GX2RLockBufferEx(&p->buffer, 0);
   if (!buffer) return -2;
   memcpy(buffer, vertices, 36 * sizeof(*vertices));
   GX2RUnlockBufferEx(&p->buffer, 0);
   GX2SetViewport(0, 0, width, height, 0, 1);
   GX2SetScissor(0, 0, width, height);
   GX2SetVertexShader(p->shader.vertexShader);
   GX2SetPixelShader(p->shader.pixelShader);
   GX2SetFetchShader(&p->shader.fetchShader);
   GX2RSetAttributeBuffer(&p->buffer, 0, sizeof(HorizonPanoramaVertex), 0);
   GX2SetPixelTexture(&p->cube, p->slot);
   GX2SetPixelSampler(&p->sampler, p->slot);
   GX2SetDepthOnlyControl(FALSE, FALSE, GX2_COMPARE_FUNC_ALWAYS);
   GX2SetAlphaTest(FALSE, GX2_COMPARE_FUNC_ALWAYS, 0.0f);
   GX2SetCullOnlyControl(GX2_FRONT_FACE_CCW, FALSE, FALSE);
   GX2SetRasterizerClipControlEx(TRUE, TRUE, TRUE);
   GX2SetColorControl(GX2_LOGIC_OP_COPY, 0, FALSE, TRUE);
   GX2SetTargetChannelMasks(GX2_CHANNEL_MASK_RGBA, (GX2ChannelMask)0, (GX2ChannelMask)0,
                            (GX2ChannelMask)0, (GX2ChannelMask)0, (GX2ChannelMask)0,
                            (GX2ChannelMask)0, (GX2ChannelMask)0);
   GX2DrawEx(GX2_PRIMITIVE_MODE_TRIANGLES, 36, 0, 1);
   return 0;
}
