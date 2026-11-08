#pragma once
#include <stddef.h>
#include <stdint.h>

typedef struct HorizonPanoramaVertex {
   float position[4];
   float direction[3];
} HorizonPanoramaVertex;

typedef struct HorizonGx2Panorama HorizonGx2Panorama;

/* GX2 must be initialized; create, draw and destroy belong on the render thread.
 * GFD stays alive until destroy. Six RGBA faces are copied in cube order.
 * Supported face sizes also depend on device texture and memory limits. */
int horizon_gx2_panorama_create(HorizonGx2Panorama **out, const void *gfd,
                                uint32_t size, const uint8_t *rgba, size_t length);
/* Uses the caller's GX2 colour target; vertices contain Horizon's projection. */
int horizon_gx2_panorama_draw(HorizonGx2Panorama *panorama, uint32_t width, uint32_t height,
                              const HorizonPanoramaVertex vertices[36]);
void horizon_gx2_panorama_destroy(HorizonGx2Panorama *panorama);
