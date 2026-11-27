# RC 49 compact terrain indices and model preparation

Translucent sections with at most 65,536 vertices use native U16 indices.
Larger sections continue to use U32. The estimate, allocation, upload
counter and draw type agree. The existing sorter still supplies all
triangles in the same order, and a rejected update preserves the previous
order. Opaque geometry follows its existing submission path.

World loading reuses the entity-model document already decoded from the
selected resource root. The existing loader still handles absent documents
and pack rollback.

## Build

| Input or output | Identity |
| --- | --- |
| Port build source | `f9a5be5624ea6281dda96d93919cbacf0d6cdcc1` |
| Engine tree | `525181edcc48d623e3dc21d9f0793044648b3556` |
| Recipe SHA-256 | `5d215c61dd626987114442a5c23b7d67f770114fefc9e1d395fe4638d0b6b53c` |
| Cafe Toolchain | `1.1.0` |
| RPX SHA-256 | `bbb33c5bfcf85f27da9b3d75122e22a66d5e5d63a8d4a3e7478ff352e8df739b` |
| WUHB SHA-256 | `50d0d75a4fb9dff5e21ac9f198f20a20034a11a45a8bb324c99993a60a2995b8` |
| Estimated Aroma code-heap headroom | 12,655,744 bytes |

The pinned builder reconstructed the exact tree and verified both package
hashes. The C buffer suite exercises the U16 boundary, U32 fallback,
invalid replacement, allocation and mapping failures, and submission type.

## Host comparisons

Five distinct camera orders expanded to identical reference indices for
both test meshes. Packed terrain vertices also matched.

| Five-camera fixture | U32 index payload | Compact index payload |
| --- | ---: | ---: |
| Open two-sided water | 61,440 B | 30,720 B |
| Mixed water, glass and ice shoreline | 28,800 B | 14,400 B |

These are index payloads across five sort orders. Stock RC 49 has a
single local player and retains one sorted stream per section. Independent
camera sort lanes have their own diagnostic work.

The exact stock and borrowed model paths produced identical 27 parsed
model layers and 23 sherd mappings in the host fixture. A chest shape
from a custom pack remained active. Across 25 interleaved warm host
runs, median model acquisition and pack projection took 37.335 ms
with a second parse and 2.058 ms with the borrowed document. Median
extra requested heap peaked at 27,708,066 and 91,349 bytes respectively.
These figures cover the model acquisition slice. Cafe frame time and
physical loading savings await matched runs.

## Offline saved-world replay

The exact RPX loaded a disposable copy of the same saved world in
interpreter Cemu using Full Performance at fixed 75%, render distance
6 and simulation distance 5. The first ready Game frame took 635.683 s.
At 672.302 s, the first paired terrain sample had one visible section
on each output. Separate 1920×1080 TV and 854×480 GamePad captures
passed before and after a 20-second stationary interval.

The GamePad input moved the player 2.334 blocks backward. Nonzero PCM,
the save barrier, the durable player position, return to Night and a
clean exit all passed. GX2 bulk free returned to 234,880,920 bytes
with a 234,880,896-byte largest block. The run qualifies this
interpreter lifecycle and the exact package. It is separate from
the physical 174-section performance workload.
