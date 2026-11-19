#include "cit_glint.h"
#include "hud_data.h"
#include <gx2/draw.h>
#include <gx2/registers.h>
#include <gx2/sampler.h>
#include <gx2/shaders.h>
#include <gx2/texture.h>
#include <math.h>
#include <stdlib.h>
#include <string.h>

int horizon_hud_texture_create(GX2Texture *texture, uint32_t width, uint32_t height,
                                 const uint8_t *pixels);
void horizon_hud_texture_destroy(GX2Texture *texture);

_Static_assert(sizeof(HorizonHudCitSlot) == 40 && _Alignof(HorizonHudCitSlot) == 4,
               "HUD CIT slot ABI changed");
_Static_assert(offsetof(HorizonHudCitSlot, factors) == 16, "HUD CIT factors offset changed");
_Static_assert(offsetof(HorizonHudCitSlot, blur) == 32, "HUD CIT blur offset changed");
_Static_assert(offsetof(HorizonHudCitSlot, blend) == 36, "HUD CIT blend offset changed");

static const GX2BlendMode factor_modes[] = {
   GX2_BLEND_MODE_ZERO,
   GX2_BLEND_MODE_ONE,
   GX2_BLEND_MODE_SRC_COLOR,
   GX2_BLEND_MODE_INV_SRC_COLOR,
   GX2_BLEND_MODE_SRC_ALPHA,
   GX2_BLEND_MODE_INV_SRC_ALPHA,
   GX2_BLEND_MODE_DST_ALPHA,
   GX2_BLEND_MODE_INV_DST_ALPHA,
   GX2_BLEND_MODE_DST_COLOR,
   GX2_BLEND_MODE_INV_DST_COLOR,
   GX2_BLEND_MODE_SRC_ALPHA_SAT,
};

typedef struct CitSlot {
   GX2Texture texture;
   float rgba[4];
   GX2BlendMode factors[4];
   uint32_t blur;
   uint32_t blend;
} CitSlot;

struct HorizonHudCit {
   GX2Sampler nearest;
   GX2Sampler linear;
   uint32_t count;
   CitSlot slots[];
};

void horizon_hud_cit_destroy(HorizonHudCit *cit)
{
   if (!cit) return;
   for (uint32_t i = 0; i < cit->count; ++i)
      horizon_hud_texture_destroy(&cit->slots[i].texture);
   free(cit);
}

HorizonHudResult horizon_hud_cit_create(HorizonHudCit **out, uint32_t width, uint32_t height,
                                         const uint8_t *pixels, size_t length,
                                         const HorizonHudCitSlot *slots, uint32_t count)
{
   if (!out) return HORIZON_HUD_INVALID;
   *out = NULL;
   size_t image_bytes;
   if (!pixels || !slots || !count || count > UINT8_MAX + 1u ||
       !width || !height || width > 16384 || height > 16384 ||
       !horizon_hud_rgba_bytes(width, height, &image_bytes) || image_bytes > length / count)
      return HORIZON_HUD_INVALID;

   for (uint32_t i = 0; i < count; ++i) {
      const HorizonHudCitSlot *slot = &slots[i];
      if (slot->blur > 1 || slot->blend > UINT8_MAX) return HORIZON_HUD_INVALID;
      for (unsigned ch = 0; ch < 4; ++ch) {
         if (!isfinite(slot->rgba[ch]) || slot->rgba[ch] < 0 ||
             slot->factors[ch] >= sizeof(factor_modes) / sizeof(factor_modes[0]))
            return HORIZON_HUD_INVALID;
      }
   }

   HorizonHudCit *cit = calloc(1, sizeof(*cit) + count * sizeof(CitSlot));
   if (!cit) return HORIZON_HUD_ALLOCATION_FAILED;
   GX2InitSampler(&cit->nearest, GX2_TEX_CLAMP_MODE_WRAP, GX2_TEX_XY_FILTER_MODE_POINT);
   GX2InitSampler(&cit->linear, GX2_TEX_CLAMP_MODE_WRAP, GX2_TEX_XY_FILTER_MODE_LINEAR);
   for (uint32_t i = 0; i < count; ++i) {
      CitSlot *slot = &cit->slots[i];
      if (!horizon_hud_texture_create(&slot->texture, width, height, pixels + i * image_bytes)) {
         horizon_hud_cit_destroy(cit);
         return HORIZON_HUD_ALLOCATION_FAILED;
      }
      memcpy(slot->rgba, slots[i].rgba, sizeof(slot->rgba));
      for (unsigned ch = 0; ch < 4; ++ch)
         slot->factors[ch] = factor_modes[slots[i].factors[ch]];
      slot->blur = slots[i].blur;
      slot->blend = slots[i].blend;
      ++cit->count;
   }
   *out = cit;
   return HORIZON_HUD_OK;
}

static uint32_t texture_slot(const HorizonHudVertex *vertex)
{
   /* Horizon's packed red channel is the numeric low byte on both endians. */
   return vertex->color & UINT8_MAX;
}

HorizonHudResult horizon_hud_cit_validate(const HorizonHudCit *cit, const HorizonHudFrame *frame,
                                           const HorizonHudRun *run)
{
   if (!cit) return HORIZON_HUD_UNSUPPORTED;
   for (uint32_t i = run->start; i < run->end; i += 3) {
      const HorizonHudVertex *triangle = &frame->vertices[i];
      const uint32_t slot = texture_slot(triangle);
      if (slot >= cit->count || cit->slots[slot].blend != run->clip[0])
         return HORIZON_HUD_INVALID;
      for (unsigned j = 0; j < 3; ++j) {
         const HorizonHudVertex *vertex = &triangle[j];
         if (texture_slot(vertex) != slot || !isfinite(vertex->glint_uv[0]) ||
             !isfinite(vertex->glint_uv[1])) return HORIZON_HUD_INVALID;
      }
   }
   return HORIZON_HUD_OK;
}

void horizon_hud_cit_draw(const HorizonHudCit *cit, const HorizonHudFrame *frame,
                           const HorizonHudRun *run, uint32_t tex_slot, uint32_t tint_uniform)
{
   for (uint32_t start = run->start; start < run->end;) {
      const uint32_t index = texture_slot(&frame->vertices[start]);
      const CitSlot *slot = &cit->slots[index];
      uint32_t end = start + 3;
      while (end < run->end && texture_slot(&frame->vertices[end]) == index)
         end += 3;

      GX2SetPixelTexture(&slot->texture, tex_slot);
      GX2SetPixelSampler(slot->blur ? &cit->linear : &cit->nearest, tex_slot);
      GX2SetPixelUniformReg(tint_uniform, 4, slot->rgba);
      GX2SetBlendControl(GX2_RENDER_TARGET_0, slot->factors[0], slot->factors[1],
                         GX2_BLEND_COMBINE_MODE_ADD, TRUE, slot->factors[2], slot->factors[3],
                         GX2_BLEND_COMBINE_MODE_ADD);
      GX2DrawEx(GX2_PRIMITIVE_MODE_TRIANGLES, end - start, start, 1);
      start = end;
   }
}
