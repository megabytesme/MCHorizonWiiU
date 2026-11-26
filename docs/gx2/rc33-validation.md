# RC 33 native candidate

## Package identity

Ben-Desktop built the hosted client in WSL from frozen port
`cdfae3d8c0562234b76f23899dbcdbdab5e2f697` with
`tools/build_cafe_game_isolated.sh --hosted-nightly`. Cafe Toolchain
1.1.0 built the pinned Horizon `0a875da91dbb0fdcb7ba8c42c11341aa229af51a`
and engine tree `b5bb7217763685f06e95847a06d90d774bcb8968`.
The source recipe SHA-256 is
`80ae2951daa8b951d7097fa94685e80ff8074404b72629d392cd5098e211e497`.

| Component | Version or identity |
| --- | --- |
| Client and patch | `0.8.0-rc.33` and `0.5.0-rc.33` |
| Builder | `0.7.0-rc.5` |
| RPX, 37,558,747 bytes | `a3e5da6f21422b5af69de5410628688be4dd09747efdb2eff4301d5dcfeb515f` |
| WUHB, 37,559,564 bytes | `59386ba05e78bede32f1b12621f1e6dc34fba7377704d4fd90af5f333c4a7153` |
| WUHB RPX offset | 512, byte-identical to the standalone RPX |
| Estimated Aroma code-heap headroom | 12,873,344 bytes |
| Prepared resources | Minecraft 26.3, schema 7, HZDATA version 2 |

The isolated builds ran in `rc33-a` and `rc33-b` under
`/var/tmp/mchorizon-rc31-milestone.U4QdI9Mv`. Each rebuilt the hosted
package in its own target directory using `--source` for the pinned
upstream checkout and `--inputs` for its respective external cache.
Both runs produced byte-identical RPX and WUHB files and passed
verification for native imports,
relocations, package hashes, the embedded RPX and code-heap sizing.
The inputs and complete build receipts are retained privately.

The private qualification archive passed preflight at its exact
RC29-Qualification guest root. Its manifest, digest and normalized
23,148-entry payload matched the known qualification copy and the
original preparation. The Cemu fixture used the same normalized
payload at the default instance's guest root.

## Verification scope

The full hosted Rust check passed on the assembled Cafe target. Native
fixtures exercised the direct and shared HUD on both displays. The
renderer coverage record gives the results for the separate repeated
frame and border scanout controls. The focused host checks exercised
camera-stable benchmark reports, 30 and 60 FPS DRS policy, ordered
mesh admission, stock and modded network recipes, archive identity
and sprite cache replacement. [Renderer coverage](coverage.md)

In a separate disposable Night fixture, the exact RPX served HTTPS with
the exported CA. Codex completed dynamic client registration and OAuth
with GamePad approval. Two independent Codex processes called the live
Night `versions` tool, and Close Horizon completed. Use a resolvable
lowercase `.local` hostname with the fixture. Night rejects uppercase
host labels during service startup. This check covered launcher access.

Automatic resolution, shared HUD submission and camera-height
mesh admission remain opt-in. The [hardware test queue](benchmarks.md#rc-32-and-rc-33-console-qualification)
covers these modes, stock Village, matched scale and geometry,
session reopening and the separate distance-12 workload.

## Saved-world replay

The exact RPX completed two existing-world sessions in one NSS-patched
Cemu process in forced-interpreter mode. TV and GamePad showed textured
terrain, the HUD and the first-person arm in both sessions. The game
reported effective 75% with 1440×809 TV and 641×360 GamePad world targets.
Scripted movement persisted through Save & Quit. The second session
kept the exact saved position. Both sessions delivered nonzero AX audio,
completed their save barriers, released the hosted workers and restored
Night. Close Horizon exited the game.

The first launch seeded a valid sprite cache with one altered fingerprint
byte. The new writer published a cache with the expected fingerprint.
The reopened world used that same cache.

| Interpreter observation | First entry with stale cache | Same-process reopen |
| --- | ---: | ---: |
| Game world-ready marker, ms | 766,189 | 738,113 |
| First serial sample with terrain on both outputs, ms | 795,297 | 766,320 |
| Network recipe preparation, ms | 21,927.140 | 25,203.313 |
| Sprite cache hits / misses | 0 / 28,641 | 28,639 / 2 |
| Rust requested high-water, bytes | 327,921,500 | 372,797,579 |
| Rust live requested after teardown, bytes | 44,315,231 | 44,449,816 |
| GX2 bulk free after save, bytes | 234,880,920 | 234,880,920 |
| GX2 largest aligned block after save, bytes | 234,880,896 | 234,880,896 |

The replay was still streaming at first visible terrain. The initial
cache conditions differed from RC 32, and host load varied across the
two runs. The timings support inspection of individual loading stages.
[Loading scorecard](loading-scorecard.json)

## Physical direct-mode qualification

Aroma loaded the verified WUHB from the qualification SD with the
existing Minecraft 26.3 preparation and its RC29-Qualification path.
Profile marker `4` selected the direct HUD. Production fixed scaling
worked at 100%, 75% and 50% without the former fixed-resolution trial
marker. The loader-to-Night serial-arrival interval was 8.084 s.
The existing CA authenticated the HTTPS service, and the owner approved
the qualification client on the GamePad before it selected the disposable
world and started the two measured sessions.

The first session reached the gameplay frame in 140.550 s and the
same-process reopen took 144.354 s. The reopen preserved the position,
yaw and pitch and reused 28,639 sprite-cache entries with two misses.
Both saves recovered 234,880,920 bytes of GX2 bulk memory with a
234,880,896-byte largest aligned block. Both sessions logged nonzero
AX PCM. Controller input opened the inventory and changed fixed scale
in game. Save & Quit returned to Night, then Close Horizon returned to
the Wii U Menu. The FAT32 card held two
distinct, readable HZP3 session snapshots, with the first retained as
`.previous` after the second publication. TV screen-pixel stills
confirmed textured gameplay and the direct inventory.
[Physical loading timings](loading-performance.md) · [Matched resolution sweep](gameplay-scorecard.md)
