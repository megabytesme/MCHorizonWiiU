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

## RC 17 performance evidence

| Captured startup span | Duration |
| --- | ---: |
| Ready world frame | 474.344 s |
| Mods/content root | 13.104 s |
| UI load, including atlas | 45.280 s |
| Core load | 237.467 s |
| Block models and sprites, inside core load | 224.227 s |
| Integrated-server open world | 24.550 s |
| Recipe synchronization, overlaps server startup | 27.068 s |

Several spans overlap. The resource archive initially missed the process's
open-file cache, then opened successfully and passed root, source and
entry validation. The 224-second composite span locates the largest
observed cost in model and sprite preparation. It does not identify the
split between SD reads, PNG decoding, model work and contention. Serial
capture is missing 2 minutes 27 seconds during loading. The 11.39 FPS
sample with 120 loaded chunks and 1,000 pending was taken during streaming.
The reported pending count combines the streaming queue, pending renderer
uploads and remote backlog. It is not a measure of any one queue or a
stationary frame-rate result.

RC 17 was linked with the AX implementation enabled. The stock mixer has
separate TV and GamePad routes in the AX voice device mix. Audible gameplay
was absent in the physical observation. That observation alone cannot
distinguish source events, PCM delivery, AX routing and output volume.
