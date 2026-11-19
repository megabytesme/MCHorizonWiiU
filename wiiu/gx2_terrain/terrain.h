#pragma once
#include <stddef.h>
#include <stdint.h>

typedef struct HorizonTerrainVertex {
   float position[4];
   float color[3];
   float uv[2];
   float cutout;
} HorizonTerrainVertex;
typedef struct HorizonGx2Terrain HorizonGx2Terrain;
typedef struct HorizonGx2TerrainSection HorizonGx2TerrainSection;
/* The callback fills the mapped vertices synchronously and does not retain
 * the pointer. A nonzero result discards the new section. */
typedef int (*HorizonTerrainFillVertices)(void *context, HorizonTerrainVertex *vertices,
                                          uint32_t count);

int horizon_gx2_terrain_create(HorizonGx2Terrain **out, const void *gfd,
                               uint32_t width, uint32_t height,
                               const uint8_t *rgba, size_t length);
void horizon_gx2_terrain_destroy(HorizonGx2Terrain *terrain);
uint32_t horizon_gx2_terrain_index_allocation_bytes(const HorizonGx2Terrain *terrain);
/* Count is the canonical four vertices per quad. On error the output handle
 * and allocation size remain zero. Empty sections do not call fill. */
int horizon_gx2_terrain_section_create(HorizonGx2TerrainSection **out,
                                      uint32_t count, HorizonTerrainFillVertices fill,
                                      void *context, uint32_t *allocation_bytes);
void horizon_gx2_terrain_wait_idle(void);
void horizon_gx2_terrain_section_destroy(HorizonGx2TerrainSection *section);
int horizon_gx2_terrain_draw(HorizonGx2Terrain *terrain,
                            HorizonGx2TerrainSection *section,
                            uint32_t target_width, uint32_t target_height,
                            uint32_t x, uint32_t y, uint32_t width, uint32_t height,
                            const float clip_rows[16], const float section_origin[4],
                            const float fog_color[4], const float fog_ranges[4]);
