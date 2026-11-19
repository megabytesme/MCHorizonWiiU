#pragma once

/* Private GX2 buffer lifecycle, shared with the host test's SDK stubs. */
enum { TERRAIN_INDEX_QUADS = (UINT16_MAX + 1) / 4 };

struct HorizonGx2TerrainSection {
   GX2RBuffer buffer;
   uint32_t vertex_count;
};

static void terrain_buffer_release(GX2RBuffer *buffer)
{
   if (GX2RBufferExists(buffer)) GX2RDestroyBufferEx(buffer, 0);
   *buffer = (GX2RBuffer){0};
}

static int terrain_buffer_create(GX2RBuffer *buffer, GX2RResourceFlags binding,
                                 uint32_t element_size, uint32_t element_count,
                                 uint32_t *allocation_bytes)
{
   const GX2RResourceFlags flags = binding | GX2R_RESOURCE_USAGE_CPU_WRITE |
      GX2R_RESOURCE_USAGE_GPU_READ;
   uint32_t alignment = GX2RGetBufferAlignment(flags);
   uint64_t payload = (uint64_t)element_size * element_count;
   if (!alignment || payload > UINT32_MAX - (alignment - 1)) return -1;
   *buffer = (GX2RBuffer){ .flags = flags, .elemSize = element_size, .elemCount = element_count };
   uint32_t bytes = GX2RGetBufferAllocationSize(buffer);
   if (bytes < payload) return -1;
   if (!GX2RCreateBuffer(buffer)) return -2;
   *allocation_bytes = bytes;
   return 0;
}

static int terrain_index_buffer_create(GX2RBuffer *buffer, uint32_t *allocation_bytes)
{
   int result = terrain_buffer_create(buffer, GX2R_RESOURCE_BIND_INDEX_BUFFER,
                                      sizeof(uint16_t), TERRAIN_INDEX_QUADS * 6,
                                      allocation_bytes);
   if (result) return result;
   uint16_t *indices = GX2RLockBufferEx(buffer, 0);
   if (!indices) return -3;
   /* Cafe writes big-endian uint16_t, selected by GX2_INDEX_TYPE_U16. */
   const uint16_t corners[] = { 0, 1, 2, 0, 2, 3 };
   for (uint32_t q = 0; q < TERRAIN_INDEX_QUADS; ++q) {
      for (uint32_t i = 0; i < 6; ++i) indices[q * 6 + i] = (uint16_t)(q * 4 + corners[i]);
   }
   GX2RUnlockBufferEx(buffer, 0);
   return 0;
}

int horizon_gx2_terrain_section_create(HorizonGx2TerrainSection **out,
                                      uint32_t count, HorizonTerrainFillVertices fill,
                                      void *context, uint32_t *allocation_bytes)
{
   if (!out || !allocation_bytes) return -1;
   *out = NULL;
   *allocation_bytes = 0;
   if (count % 4 || (count && !fill)) return -1;
   /* The fill callback also borrows the mapped bytes as a Rust slice. */
#if PTRDIFF_MAX < UINT32_MAX
   if (count > PTRDIFF_MAX / sizeof(HorizonTerrainVertex))
      return -1;
#endif
   HorizonGx2TerrainSection *section = calloc(1, sizeof(*section));
   if (!section) return -2;
   section->vertex_count = count;
   uint32_t bytes = 0;
   int result = 0;
   if (count) {
      result = terrain_buffer_create(&section->buffer, GX2R_RESOURCE_BIND_VERTEX_BUFFER,
                                     sizeof(HorizonTerrainVertex), count, &bytes);
      if (result) goto fail;
      HorizonTerrainVertex *vertices = GX2RLockBufferEx(&section->buffer, 0);
      if (!vertices) { result = -3; goto fail; }
      result = fill(context, vertices, count);
      GX2RUnlockBufferEx(&section->buffer, 0);
      if (result) { result = -4; goto fail; }
   }
   *out = section;
   *allocation_bytes = bytes;
   return 0;
fail:
   /* The buffer was never submitted, so it does not have to wait for GX2. */
   terrain_buffer_release(&section->buffer);
   free(section);
   return result;
}

void horizon_gx2_terrain_section_destroy(HorizonGx2TerrainSection *section)
{
   if (!section) return;
   if (GX2RBufferExists(&section->buffer)) GX2DrawDone();
   terrain_buffer_release(&section->buffer);
   free(section);
}

static void terrain_draw_indexed(GX2RBuffer *quad_indices, uint32_t vertex_count)
{
   uint32_t remaining = vertex_count / 4;
   uint32_t base_vertex = 0;
   while (remaining) {
      uint32_t quads = remaining < TERRAIN_INDEX_QUADS ? remaining : TERRAIN_INDEX_QUADS;
      GX2RDrawIndexed(GX2_PRIMITIVE_MODE_TRIANGLES, quad_indices, GX2_INDEX_TYPE_U16,
                      quads * 6, 0, base_vertex, 1);
      remaining -= quads;
      base_vertex += quads * 4;
   }
}
