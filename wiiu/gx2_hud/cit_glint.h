#pragma once
#include "hud.h"

typedef struct HorizonHudCit HorizonHudCit;

/* C ABI shared with the Rust HUD bridge. Blend factors are RGB source and
 * destination, then alpha source and destination. */
typedef struct HorizonHudCitSlot {
   float rgba[4];
   uint32_t factors[4];
   uint32_t blur;
   uint32_t blend;
} HorizonHudCitSlot;

/* Slot images are tightly packed RGBA8, one width-by-height image per slot.
 * Creation copies the images and descriptors. */
HorizonHudResult horizon_hud_cit_create(HorizonHudCit **out, uint32_t width, uint32_t height,
                                         const uint8_t *pixels, size_t length,
                                         const HorizonHudCitSlot *slots, uint32_t count);
/* The caller must finish pending GPU reads before destroying the slots. */
void horizon_hud_cit_destroy(HorizonHudCit *cit);

/* Common frame geometry validation must succeed before validating a CIT run. */
HorizonHudResult horizon_hud_cit_validate(const HorizonHudCit *cit, const HorizonHudFrame *frame,
                                           const HorizonHudRun *run);
/* The caller binds the common HUD state, selects the CIT shader path, and
 * restores the regular glint texture and sampler after drawing. */
void horizon_hud_cit_draw(const HorizonHudCit *cit, const HorizonHudFrame *frame,
                           const HorizonHudRun *run, uint32_t tex_slot, uint32_t tint_uniform);
