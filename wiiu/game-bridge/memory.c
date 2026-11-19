#include <coreinit/debug.h>
#include <coreinit/memdefaultheap.h>
#include <coreinit/memexpheap.h>
#include <coreinit/memheap.h>
#include <gx2/surface.h>
#include <stdint.h>
#include <whb/gfx.h>

static MEMHeapHandle default_heap;

void horizon_wiiu_memory_init(void)
{
   /* MEM2's base heap can have zero free bytes while a child heap serves
    * default allocations. Accept the containing heap only after a known
    * allocation changes its free count. Do this before workers are started. */
   void *small = MEMAllocFromDefaultHeapEx(64, 64);
   if (!small) return;
   MEMHeapHandle heap = MEMFindContainHeap(small);
   uint32_t before = 0, during = 0, same_heap = 0;
   if (heap && heap->tag == MEM_EXPANDED_HEAP_TAG &&
       (heap->flags & MEM_HEAP_FLAG_USE_LOCK)) {
      before = MEMGetTotalFreeSizeForExpHeap(heap);
      const uint32_t probe_bytes = 64 * 1024;
      void *probe = MEMAllocFromDefaultHeapEx(probe_bytes, 64);
      if (probe) {
         during = MEMGetTotalFreeSizeForExpHeap(heap);
         same_heap = MEMFindContainHeap(probe) == heap;
         if (same_heap && before >= during && before - during >= probe_bytes)
            default_heap = heap;
         MEMFreeToDefaultHeap(probe);
      }
   }
   MEMFreeToDefaultHeap(small);
   if (default_heap) {
      uint32_t span = (uint32_t)((uintptr_t)default_heap->dataEnd - (uintptr_t)default_heap->dataStart);
      OSReport("Horizon memory default_heap arena=%u span=%u free=%u largest_64=%u\n",
               (unsigned)MEMGetArena(default_heap), span,
               MEMGetTotalFreeSizeForExpHeap(default_heap),
               MEMGetAllocatableSizeForExpHeapEx(default_heap, 64));
   } else {
      OSReport("Horizon memory default_heap=unverified found=%u tag=%08x flags=%08x probe_before=%u probe_during=%u probe_same_heap=%u\n",
               heap != NULL, heap ? (unsigned)heap->tag : 0, heap ? heap->flags : 0,
               before, during, same_heap);
   }
}

void horizon_wiiu_memory_report(const char *label, uint32_t len, uint32_t live,
                               uint32_t peak, uint32_t blocks)
{
   if (default_heap) {
      OSReport("Horizon memory %.*s rust_live=%u rust_peak=%u rust_blocks=%u heap_free=%u largest_4=%u largest_64=%u\n",
               (int)len, label, live, peak, blocks, MEMGetTotalFreeSizeForExpHeap(default_heap),
               MEMGetAllocatableSizeForExpHeapEx(default_heap, 4),
               MEMGetAllocatableSizeForExpHeapEx(default_heap, 64));
   } else {
      OSReport("Horizon memory %.*s rust_live=%u rust_peak=%u rust_blocks=%u heap_free=unverified\n",
               (int)len, label, live, peak, blocks);
   }
}

void horizon_wiiu_memory_alloc_failure(uint32_t bytes, uint32_t align, uint32_t live,
                                      uint32_t peak, uint32_t blocks)
{
   /* System has returned from the failed allocation and released its heap lock. */
   if (default_heap) {
      OSReport("Horizon memory alloc_failed bytes=%u align=%u rust_live=%u rust_peak=%u rust_blocks=%u heap_free=%u largest_aligned=%u\n",
               bytes, align, live, peak, blocks, MEMGetTotalFreeSizeForExpHeap(default_heap),
               MEMGetAllocatableSizeForExpHeapEx(default_heap, align < 4 ? 4 : (int)align));
   } else {
      OSReport("Horizon memory alloc_failed bytes=%u align=%u rust_live=%u rust_peak=%u rust_blocks=%u heap_free=unverified\n",
               bytes, align, live, peak, blocks);
   }
}

void horizon_wiiu_memory_report_display_surfaces(void)
{
   const GX2ColorBuffer *tv = WHBGfxGetTVColourBuffer();
   const GX2ColorBuffer *drc = WHBGfxGetDRCColourBuffer();
   const GX2DepthBuffer *tv_depth = WHBGfxGetTVDepthBuffer();
   const GX2DepthBuffer *drc_depth = WHBGfxGetDRCDepthBuffer();
   OSReport("Horizon memory gx2_display tv_color=%u tv_depth=%u drc_color=%u drc_depth=%u\n",
            tv ? tv->surface.imageSize : 0, tv_depth ? tv_depth->surface.imageSize : 0,
            drc ? drc->surface.imageSize : 0, drc_depth ? drc_depth->surface.imageSize : 0);
}
