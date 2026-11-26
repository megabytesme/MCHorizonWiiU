# Cafe default-heap diagnostic

This standalone app measures the `MEMAllocFromDefaultHeapEx` and `MEMFreeToDefaultHeap` calls used by the hosted game's `System` allocator. It uses a bounded synthetic workload and leaves game data, settings, and the hosted app untouched. Build using the pinned Cafe SDK from the port checkout.

```sh
bash wiiu/diagnostics/allocator/build.sh "$scratch/allocator"
```

Launch `HorizonAllocator.wuhb` from its own Aroma application directory. Capture serial output from `Horizon allocator timer_hz` through `Horizon allocator fixture PASSED`. The app exits after the last record without starting graphics, audio, or networking.

Each worker holds 96 blocks per batch and runs 16 batches. The six layouts range from 64 bytes aligned to 4 through 32,768 bytes aligned to 64. It changes free order between batches. The pair runs use separate Espresso cores. In `contended`, each worker frees its own allocations. In `cross_free`, both workers finish allocating, then each frees the other worker's batch. Both frees complete before either worker reuses its pointer list. The reports check allocation alignment, the expected number of allocations and frees, and overlap between the paired workers. Event and thread setup times are outside the per-operation timing samples.

The totals are hexadecimal timer ticks. Divide a total by its operation count and by `timer_hz` for mean seconds per call. The maximum is in ticks as well. Compare those numbers with the back-to-back timer baseline and use `overlap_us` to check the pair phases. `heap_free` and `largest_64` describe the confirmed default expandable heap at each checkpoint. The timed calls include allocator work, lock waiting, and any preemption between the two timestamp reads. They do not attribute mutex wait time separately. A physical run measures these synthetic layouts. Allocator samples from cold loading, warm reopening and gameplay are still needed to weight those costs by the game's actual allocation patterns.

## Verification

Two standalone Cafe builds in separate output directories passed with `-Wall -Wextra -Werror` and produced byte-identical packages. The disposable forced-interpreter Cemu run reported 30 layout/core records with 256 allocations and 256 frees each. Both paired modes ran on distinct cores with observed overlap, and every mode reported zero failures. Cemu returned zero for the free-space fields, so the run provides execution evidence only. Physical allocator timing and fragmentation remain pending.

| Artifact | SHA-256 |
|---|---|
| `HorizonAllocator.rpx` | `a19b2969e3c307c9ab01f32ebe0ab078726e5c696416d2413b2d01f26198e6f6` |
| `HorizonAllocator.wuhb` | `f647ee322c369d7d47f273efc20ba2fbfa44d8843e37e4745e9ede44e43b3d04` |
