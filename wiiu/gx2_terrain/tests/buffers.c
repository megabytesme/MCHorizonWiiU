/* Host regression: cc -std=c11 -Wall -Wextra -Werror buffers.c -o buffers && ./buffers */
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include "../terrain.h"

/* Stub only the SDK operations used by the production buffer code. */
typedef uint32_t GX2RResourceFlags;
typedef struct {
   GX2RResourceFlags flags;
   uint32_t elemSize, elemCount;
   void *buffer;
} GX2RBuffer;
enum {
   GX2R_RESOURCE_USAGE_CPU_WRITE = 1,
   GX2R_RESOURCE_USAGE_GPU_READ = 2,
   GX2R_RESOURCE_BIND_VERTEX_BUFFER = 4,
   GX2R_RESOURCE_BIND_INDEX_BUFFER = 8,
   GX2_PRIMITIVE_MODE_TRIANGLES = 4,
   GX2_INDEX_TYPE_U16 = 4,
};

static bool fail_calloc, fail_create, fail_map, reject_fill;
static unsigned sections, buffers, mapped, waits, fills;
static unsigned draw_count;
static struct { uint32_t index_count, base_vertex; } draws[3];

static void *section_calloc(size_t count, size_t bytes)
{
   if (fail_calloc) return NULL;
   void *p = calloc(count, bytes);
   assert(p);
   ++sections;
   return p;
}

static void section_free(void *p)
{
   assert(p && sections);
   free(p);
   --sections;
}

static bool GX2RBufferExists(GX2RBuffer *buffer)
{
   return buffer->buffer != NULL;
}

static uint32_t GX2RGetBufferAlignment(GX2RResourceFlags flags)
{
   assert(flags & GX2R_RESOURCE_USAGE_CPU_WRITE);
   assert(flags & GX2R_RESOURCE_USAGE_GPU_READ);
   return 64;
}

static uint32_t GX2RGetBufferAllocationSize(GX2RBuffer *buffer)
{
   uint32_t bytes = buffer->elemSize * buffer->elemCount;
   return (bytes + 63) & ~63u;
}

static bool GX2RCreateBuffer(GX2RBuffer *buffer)
{
   if (fail_create) return false;
   buffer->buffer = malloc(GX2RGetBufferAllocationSize(buffer));
   assert(buffer->buffer);
   ++buffers;
   return true;
}

static void GX2RDestroyBufferEx(GX2RBuffer *buffer, unsigned flags)
{
   assert(GX2RBufferExists(buffer) && flags == 0 && !mapped && buffers);
   free(buffer->buffer);
   --buffers;
}

static void *GX2RLockBufferEx(GX2RBuffer *buffer, unsigned flags)
{
   assert(GX2RBufferExists(buffer) && flags == 0 && !mapped);
   if (fail_map) return NULL;
   ++mapped;
   return buffer->buffer;
}

static void GX2RUnlockBufferEx(GX2RBuffer *buffer, unsigned flags)
{
   assert(GX2RBufferExists(buffer) && flags == 0 && mapped == 1);
   --mapped;
}

static void GX2DrawDone(void) { ++waits; }

static void GX2RDrawIndexed(unsigned mode, GX2RBuffer *buffer, unsigned type,
                           uint32_t count, uint32_t offset, uint32_t base, unsigned instances)
{
   assert(mode == GX2_PRIMITIVE_MODE_TRIANGLES && type == GX2_INDEX_TYPE_U16);
   assert(buffer->flags & GX2R_RESOURCE_BIND_INDEX_BUFFER);
   assert(!mapped && instances == 1 && offset == 0);
   assert(count <= buffer->elemCount && draw_count < 3);
   draws[draw_count].index_count = count;
   draws[draw_count++].base_vertex = base;
}

#define calloc section_calloc
#define free section_free
#include "../terrain_buffers.h"
#undef calloc
#undef free

static int fill(void *context, HorizonTerrainVertex *dest, uint32_t count)
{
   assert(mapped && count == 4);
   ++fills;
   for (unsigned i = 0; i < count; ++i) {
      dest[i] = (HorizonTerrainVertex){ .position = { *(const float *)context, i, 0, 1 } };
   }
   return reject_fill ? -1 : 0;
}

static void check_index_groups(void)
{
   GX2RBuffer indices = {0};
   uint32_t bytes = 0;
   assert(terrain_index_buffer_create(&indices, &bytes) == 0);
   assert(bytes == 196608 && buffers == 1 && !mapped);
   const uint16_t *entries = indices.buffer;
   for (unsigned q = 0; q < TERRAIN_INDEX_QUADS; ++q) {
      const unsigned expected[] = { 0, 1, 2, 0, 2, 3 };
      for (unsigned i = 0; i < 6; ++i) assert(entries[q * 6 + i] == q * 4 + expected[i]);
   }
   assert(entries[indices.elemCount - 1] == UINT16_MAX);
   terrain_draw_indexed(&indices, 0);
   assert(draw_count == 0);
   terrain_draw_indexed(&indices, TERRAIN_INDEX_QUADS * 4);
   assert(draw_count == 1 && draws[0].index_count == 98304 && draws[0].base_vertex == 0);
   draw_count = 0;
   terrain_draw_indexed(&indices, (TERRAIN_INDEX_QUADS + 1) * 4);
   assert(draw_count == 2 && draws[1].index_count == 6 && draws[1].base_vertex == 65536);
   draw_count = 0;
   terrain_draw_indexed(&indices, (TERRAIN_INDEX_QUADS * 2 + 3) * 4);
   assert(draw_count == 3 && draws[2].index_count == 18 && draws[2].base_vertex == 131072);
   terrain_buffer_release(&indices);
   assert(!buffers);

   fail_map = true;
   assert(terrain_index_buffer_create(&indices, &bytes) == -3);
   terrain_buffer_release(&indices);
   assert(!buffers && !mapped);
   fail_map = false;
}

static void check_upload_lifetimes(void)
{
   HorizonGx2TerrainSection *empty;
   uint32_t bytes = UINT32_MAX;
   assert(horizon_gx2_terrain_section_create(&empty, 0, NULL, NULL, &bytes) == 0);
   assert(sections == 1 && !bytes && !buffers && !fills);
   horizon_gx2_terrain_section_destroy(empty);
   assert(!sections && !waits);

   float old_x = -1.5f, new_x = 4.5f;
   HorizonGx2TerrainSection *original;
   assert(horizon_gx2_terrain_section_create(&original, 4, fill, &old_x, &bytes) == 0);
   assert(sections == 1 && buffers == 1 && fills == 1 && bytes == 192 && !mapped);

   HorizonGx2TerrainSection *replacement = original;
   assert(horizon_gx2_terrain_section_create(&replacement, 5, fill, &new_x, &bytes) == -1);
   assert(!replacement && !bytes);
   assert(horizon_gx2_terrain_section_create(&replacement, UINT32_MAX & ~3u, fill, &new_x, &bytes) == -1);
   assert(!replacement && !bytes && sections == 1 && buffers == 1);
   assert(horizon_gx2_terrain_section_create(&replacement, 4, NULL, &new_x, &bytes) == -1);
   fail_calloc = true;
   assert(horizon_gx2_terrain_section_create(&replacement, 4, fill, &new_x, &bytes) == -2);
   fail_calloc = false;
   fail_create = true;
   assert(horizon_gx2_terrain_section_create(&replacement, 4, fill, &new_x, &bytes) == -2);
   fail_create = false;
   fail_map = true;
   assert(horizon_gx2_terrain_section_create(&replacement, 4, fill, &new_x, &bytes) == -3);
   fail_map = false;
   reject_fill = true;
   assert(horizon_gx2_terrain_section_create(&replacement, 4, fill, &new_x, &bytes) == -4);
   reject_fill = false;
   assert(!replacement && !bytes && sections == 1 && buffers == 1 && !mapped && !waits);
   assert(((HorizonTerrainVertex *)original->buffer.buffer)[0].position[0] == old_x);

   assert(horizon_gx2_terrain_section_create(&replacement, 4, fill, &new_x, &bytes) == 0);
   assert(bytes == 192 && sections == 2 && buffers == 2 && !mapped);
   horizon_gx2_terrain_section_destroy(original);
   assert(sections == 1 && buffers == 1 && waits == 1);
   assert(((HorizonTerrainVertex *)replacement->buffer.buffer)[0].position[0] == new_x);
   horizon_gx2_terrain_section_destroy(replacement);
   assert(!sections && !buffers && waits == 2);
}

int main(void)
{
   check_index_groups();
   check_upload_lifetimes();
   puts("Indexed terrain buffer tests passed");
   return 0;
}
