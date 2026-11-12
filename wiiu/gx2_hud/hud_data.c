#include "hud_data.h"
#include <math.h>

int horizon_hud_rgba_bytes(uint32_t width, uint32_t height, size_t *bytes)
{
   if (!bytes || (height && (size_t)width > SIZE_MAX / 4 / (size_t)height)) return 0;
   *bytes = (size_t)width * (size_t)height * 4;
   return 1;
}

HorizonHudResult horizon_hud_validate(const HorizonHudFrame *f, uint32_t width,
                                       uint32_t height, uint32_t capacity)
{
   if (!f || !width || !height || !f->viewport_width || !f->viewport_height ||
       !isfinite(f->scale) || f->scale <= 0 || f->vertex_count > capacity ||
       (f->vertex_count && !f->vertices) || (f->run_count && !f->runs) ||
       (f->patch_count && !f->patches)) return HORIZON_HUD_INVALID;
   if (f->pack_text_shader) return HORIZON_HUD_UNSUPPORTED;
   uint32_t next = 0;
   for (uint32_t i = 0; i < f->run_count; ++i) {
      const HorizonHudRun *r = &f->runs[i];
      if (r->start != next || r->end < r->start || r->end > f->vertex_count || r->start % 3 || r->end % 3)
         return HORIZON_HUD_INVALID;
      if (r->blend != HORIZON_HUD_ALPHA && r->blend != HORIZON_HUD_INVERT &&
          r->blend != HORIZON_HUD_ADDITIVE)
         return HORIZON_HUD_UNSUPPORTED;
      next = r->end;
   }
   if (next != f->vertex_count) return HORIZON_HUD_INVALID;
   for (uint32_t i = 0; i < f->vertex_count; ++i) {
      const HorizonHudVertex *v = &f->vertices[i];
      if (!isfinite(v->pos[0]) || !isfinite(v->pos[1]) || !isfinite(v->uv[0]) || !isfinite(v->uv[1]) ||
          !isfinite(v->depth) || !isfinite(v->pos[0] * f->scale) || !isfinite(v->pos[1] * f->scale) ||
          !isfinite(v->pos[0] * f->scale / f->viewport_width * 2.0f - 1.0f) ||
          !isfinite(1.0f - v->pos[1] * f->scale / f->viewport_height * 2.0f) ||
          v->depth < 0 || v->depth > 1) return HORIZON_HUD_INVALID;
      if (v->uv[0] >= 2.0f) return HORIZON_HUD_UNSUPPORTED;
   }
   for (uint32_t i = 0; i < f->patch_count; ++i) {
      const HorizonHudPatch *p = &f->patches[i];
      size_t bytes;
      if (!p->rgba || !p->width || !p->height || p->x > width || p->y > height || p->width > width - p->x ||
          p->height > height - p->y || !horizon_hud_rgba_bytes(p->width, p->height, &bytes) ||
          bytes > p->length)
         return HORIZON_HUD_INVALID;
   }
   return HORIZON_HUD_OK;
}

void horizon_hud_encode_vertices(const HorizonHudFrame *f, HorizonHudGpuVertex *out)
{
   for (uint32_t i = 0; i < f->vertex_count; ++i) {
      const HorizonHudVertex *v = &f->vertices[i];
      out[i].position[0] = v->pos[0] * f->scale / f->viewport_width * 2.0f - 1.0f;
      out[i].position[1] = 1.0f - v->pos[1] * f->scale / f->viewport_height * 2.0f;
      out[i].position[2] = 0.0f; out[i].position[3] = 1.0f;
      /* Horizon's numeric color is AABBGGRR, even on big-endian PowerPC. */
      for (unsigned c = 0; c < 4; ++c) out[i].color[c] = (float)((v->color >> (c * 8)) & 255) / 255.0f;
      out[i].uv[0] = v->uv[0]; out[i].uv[1] = v->uv[1];
   }
}
