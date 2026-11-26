# RC 41 development candidate

RC 41 builds against the pinned Horizon
`0a875da91dbb0fdcb7ba8c42c11341aa229af51a` source with Cafe Toolchain 1.1.0.
The builder transferred the RC 40 Cargo target without duplicating it.
Release Cargo took 20m 45s. Native relocation, code heap and package hash
checks passed. The established forced-interpreter Cemu reached Night's
profile home screen and captured both displays.

| Component | Exact value |
| --- | --- |
| Engine tree | `598a9678ea46d7f13463a7dcb8cfcf209944a7aa` |
| Recipe SHA-256 | `1b2658a2c6baa2c458a1af8aaf725d448e48a1c2795cec4bf698ed3b728a14a0` |
| RPX SHA-256 | `a1195b54d75715224c7827cf762fb491094282d4ef9454a3724d167409c7694f` |
| WUHB SHA-256 | `6e3d159782f6da82896dec80a067c704acea12c479276027268332b361e08980` |
| Estimated Aroma code heap headroom | 12,801,088 bytes |

## Candidate changes

- Authenticated gameplay now exposes stock graphics reads and reviewed
  writes through `game_options_get` and `game_options_set`. The same
  settings handlers persist preset and fixed-scale changes. Automatic
  resolution remains subject to the existing trial gate. Native interactive
  verification is pending. [MCP usage](../mcp/README.md#live-graphics-in-the-hosted-development-recipe)
- The villager hat texture scan searches four relevant directories.
  Host loose, indexed and nested ZIP fixtures matched the expected
  metadata. On the synthetic 4,096-texture input, median lookup fell from
  72.132 to 15.929 ms across 21 matched runs.
- Stock Fusion texture discovery can be skipped when the unchanged
  model catalogue and its blockstate references prove it has no applicable
  connecting model. The host fixture checked 4,192 model definitions and
  2,603 references. The median of 11 isolated matched warm runs fell from
  65.121 to 0.996 ms. Pack override and unknown-input tests exercise the
  existing discovery path. Physical stage timings remain pending.
- Marker `4b` enables back-face culling for solid and cutout terrain.
  Marker `4` keeps the control. The same standalone RPX passed 16 pixel
  checks per marker across TV and GamePad. Both modes submit the same
  index counts, so compare measured cadence and visual correctness in
  the unchanged saved view. [Winding fixture](../../wiiu/diagnostics/gx2-terrain-winding/README.md)

## Hosted `4b` saved-world replay

The exact RPX completed one saved-world round in established forced-interpreter
Cemu. The isolated fixture used fixed 75%, DRS off, render distance 12 and
simulation distance 12. All 28,171 copied resource files matched their
source SHA-256 hashes. Serial confirmed the solid and cutout back-face trial.

The first Game frame returned after 509.317 s. The terrain gate reached
10 sections on each output after 738.875 s. Distinct 1920×1080 TV and
854×480 GamePad stills showed terrain, cutout vegetation, the HUD and
first-person hand. HZP3 retained 96 gameplay pairs outside native screenshot
rows. The final 15 pairs submitted 10–18 sections on each output with
world targets of 1440×809 and 641×360. The session capture SHA-256 is
`d1e7cae299ef1a76b420b51a95ce1b8e86725364f5b394e7f8f71117b74adace`.

The backward input moved the saved player 4.313 blocks along negative Z.
The save returned to Night with 234,880,920 free GX2 bulk bytes and a
234,880,896-byte largest free block. AX delivered nonzero PCM and its
final read and write counters agreed. Night exited with `coreinit.exit(0)`.

This replay exercises a single save and the trial renderer. Matched
control pixels, throughput, native settings, reopen and hardware acceptance
remain pending. The physical reference stays at RC 38.
