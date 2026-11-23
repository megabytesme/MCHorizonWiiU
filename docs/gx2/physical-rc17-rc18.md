# Physical Cafe save investigation

RC 17 booted Horizon Night on the real Wii U and opened a disposable Minecraft
26.3 world. The first ready game frame was logged after 474,344 ms. Save &
Quit acknowledged the request, then left both displays on the save screen.

## Two independent filesystem failures

The failed save left complete staged `*_new` files on the SD for weather,
player NBT, advancements and statistics. Existing target files or previous
NBT backups were still present. Rust std forwarded rename requests to WUT
without POSIX-style replacement. The standalone probe returned `-196630`
when renaming a closed, flushed stage onto an existing file. Moving the live
file aside, publishing into the vacant name and reading both files succeeded.

A separate raw FSA probe opened an empty file in `w+` mode and sought to
byte 8192. Hardware returned `-196651` (`OUT_OF_RANGE`), the same status
and attempted position logged during the failed RC 17 save. The Anvil
writer reserves two 4096-byte header sectors in its bitmap before its first
payload write, including when the file already exists but is shorter than
the header. RC 17 sought to the first allocated sector without first
establishing the necessary file length. The failure log identifies a
handle, so it does not identify the exact affected region file. The probe
confirmed that writing the header first permits the seek, append, truncate
and flush sequence. The replacement failures occurred before the fatal
seek and the probe reproduced the two operations separately.

The same probe on Cemu accepted both rename-over-existing and the seek on
an empty file. These are relevant emulator divergences. The physical probe
also reported a successful SD-volume flush. The probe application exited
with status zero. The console stayed on the launch screen afterward and
responded to normal shutdown.

A read-only Windows FAT scan after the original stalled test found two lost
allocation chains totaling 288 KiB. This confirms an inconsistent FAT
directory or allocation state after the incident. It does not establish
when the chains were lost or a defect in the media or reader. No repair
operation was run.

## RC 18 save path

Cafe writes the whole payload into an exclusively reserved same-volume
staging directory, syncs the file and closes its handle. It moves the live
file to the recovery name, flushes the volume, publishes the stage into the
vacant live name, and flushes the volume before retiring any transient
recovery file. NBT retains its ordinary `_old` generation. A missing live
name is readable from recovery. Readers and writers share a transaction
lock, and each game retains its existing world and region locking. A failed
publication attempts to restore the old name and returns the error. A
failed volume flush keeps the recovery data and returns the error.

Anvil extends a short file before seeking to a payload sector. The Cafe
standard library zero-fills extensions and only calls the native seek
within the established extent. The owner statistics and advancement save
errors propagate into Cafe's save path.

Host tests use the Cafe rename precondition and cover first and repeated
publication, interrupted replacement, collision ownership, overlapping
writes, rollback and volume-flush errors, as well as short-file seek and
truncation. See the [WUT probe](../../wiiu/fs/probe) for the independent
primitive checks. Release acceptance requires a physical save, return to
Night, reopen, and verification of the persisted fixture change.

## Exact RC 18 emulator acceptance

Two isolated native builds matched for RPX, WUHB, Cargo lock and source and
build receipts. The WUHB contains the exact RPX. The Aroma code-heap
estimate, including compressed relocations, leaves 13,167,104 bytes of
headroom. The assembled source tree is
`b463d2ea314e619a20062424402a618a33c2711e`. The RPX SHA-256 is
`3417cec7f12389e715266c115ad31e9bf701e1b8c0586515658e3b900a025792`.

Cemu ran this exact build with the disposable fixture at distance 12. The
initial test moved the player while the server still reported terrain
loading. Save & Quit returned to Night, but reopening and direct NBT
inspection both showed the previous player position. The cause of the
early-input discrepancy remains unverified.

The completed retry waited for the world clock and terrain streaming to
advance, then moved the player. Both displays showed gameplay. The first
save completed, Night and HTTPS MCP returned, and the saved NBT record
contained the new position. Reopening reported that same position. A second
save again returned to Night and left MCP reachable. Both sessions finished
with 234,880,920 bytes free in the GX2 bulk heap. Live Rust allocations after
the two game drops were 44,416,253 and 44,452,519 bytes. First world entry
took 911.213 seconds and reopening took 826.288 seconds in this Cemu
interpreter run.

## Exact RC 18 physical acceptance

The verified RC 18 WUHB passed the Aroma loader and entered Night on TV and
GamePad. The console's public CA matched its displayed SHA-256 fingerprint.
GamePad-approved native HTTPS MCP confirmed the client version and prepared
Minecraft 26.3 resources before a separate gameplay grant.

The disposable fixture rendered on both outputs and responded to GamePad
input. After terrain streaming and server ticks had advanced, the test
changed the player position and requested Save & Quit through MCP. Serial
reported the save barrier complete and Night returned. The bulk heap
recovered to 234,880,920 bytes free, and native MCP still returned version
information in Night. The fixture reopened at the exact saved position. The
owner left the reopened fixture running. No filesystem fatal, out-of-range
seek, or fatal memory condition was observed in the capture through
verification. The physical save and reopen gate passed.

| RC 18 hardware startup span | First entry | Reopen in the same process |
| --- | ---: | ---: |
| First original game frame with world ready | 472.381 s | 461.281 s |
| UI atlas | 44.923 s | 46.116 s |
| Core total | 235.982 s | 234.599 s |
| Block models and sprites, inside core | 222.808 s | 223.128 s |
| Model baking, inside block models and sprites | 6.234 s | 6.098 s |
| World preload | 80.906 s | 58.493 s |
| Integrated-server world open | 24.016 s | 24.271 s |
| Recipe synchronization | 26.761 s | 27.156 s |

Spans overlap. Model baking began more than 206 seconds into the composite
block-model and sprite span on first entry. On reopen that span still took
223 seconds. The largest phase remains in the warm path and needs
instrumented hardware timing below the composite span.

At distance 12 the first session's streaming sample showed 12.38 FPS,
172 client chunks and 656 renderer uploads pending. A reopen sample showed
26.74 FPS, 637 client chunks and 532 renderer uploads pending. The camera
moved during the test. These samples establish the streaming workload,
while steady-state frame-time percentiles remain unmeasured.

Neither TV nor GamePad speakers played footsteps in the hardware test.
MCP counted sand footstep events with an empty decoder and mixer. The RC 18
startup log reported an empty sound registry. The verified prepared archive
omits `assets/minecraft/sounds.json` and the runtime treats ordinary archive
misses as definitive. That masks the separately imported loose registry,
even though the loose Ogg files exist. A focused host repair test recovered
the stock registry and all five sand footstep files through the
archive-backed lookup. AX delivery and speaker output still require a native
validation run with that repair.

## Extended RC 18 physical session

The reopened fixture later showed the in-game death screen. On reconnection,
the existing native HTTPS endpoint timed out and no fresh gameplay state or
frame timing was available. Serial showed the client still had 637 columns
at distance 12 while retained section buffers increased from 154,351,104
to 226,485,888 allocation bytes. The upload backlog was 414 in the earlier
sample and 560 in the final sample, after repeated allocation failures.
The queue did not settle in the recorded interval.

The last failure was a 379,840-byte, 64-byte-aligned allocation in the region
writer's zlib compressor. The bulk heap had 5,867,396 bytes free in total and
a largest aligned block of 280,044 bytes. The default-heap probe was
unavailable. The codec reported `MemError`, panicked, and CafeOS recorded
an illegal-instruction exception. This is a separate memory-pressure
failure during the open session. The first manual Save & Quit and reopen
still passed. The trace contains no second manual save or second durable
save barrier. The console was left in its observed state.

## RC 17 performance evidence

| Captured startup span | Duration |
| --- | ---: |
| Ready world frame | 474.344 s |
| Mods/content root | 13.104 s |
| UI load | 45.280 s |
| UI atlas, inside UI load | 45.253 s |
| Core load | 237.467 s |
| Block models and sprites, inside core load | 224.227 s |
| Block models and sprites begins to model baking begins | 206.730 s |
| Model baking | 7.396 s |
| Integrated-server open world | 24.550 s |
| Recipe synchronization, overlaps server startup | 27.068 s |

Several spans overlap. The resource archive initially missed the process's
open-file cache, then opened successfully and passed root, source and
entry validation. Within the largest observed span, 206.730 seconds passed
before model baking began. The preceding path includes sprite-cache
fingerprinting, cache reads, initial sprite preparation and connected-texture
rule discovery. Separate phase timing is needed to assign the time to those
operations. The loose vanilla texture tree had 4,182 files. The cache's
directory fingerprint can enumerate and stat the loose files even when
resource payloads are served by the prepared archive. Serial capture is
missing 2 minutes 27 seconds during loading.

The 11.39 FPS sample was taken during streaming at distance 12, with 120
client chunks loaded. Of the 1,000 reported pending jobs, 999 were renderer
uploads and one was queued in the streaming pipeline. Active generation,
lighting and meshing were zero in that sample. The mesher's admission check
computes remaining room in sections but applies it to a list of columns
before submitting a job for each section. A tall enough batch can exceed the
intended section limit. The upload queue is visible in the sample and the
admission mismatch is visible in the source. The trace alone cannot assign
how much of the backlog or frame latency came from that mismatch.

Later streaming samples recorded up to 334 resident client columns, 422
server columns, 1,213 pending mesh uploads and 378,959,005 bytes of peak
live Rust allocations. A late sample held 2,146 native terrain sections in
57,795,648 bytes of vertex allocations. The camera moved during this
capture. Frame-time percentiles for a warmed and stationary world remain
unmeasured.

RC 17 was linked with the AX implementation enabled. The stock mixer has
separate TV and GamePad routes in the AX voice device mix. Audible gameplay
was absent in the physical observation. The MCP sample recorded a sand
footstep and ambient events while reporting zero decoded clips and zero
active mixer channels. Stock audio files were present on the SD and game
volume categories were enabled. Successful voice allocation, callback
delivery and output from each speaker still require separate checks.
