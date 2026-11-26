# Standalone game profiles and x64 recompiler diagnostics

Cemu 2.6 loads a game profile when starting an installed title. Its standalone
RPX path derives a placeholder title ID but skips profile loading, so writing
`cpuMode = 1` in a portable profile alone can still run with three emulated
cores. Apply [the standalone profile patch](standalone-profile-cemu-2.6.patch)
to a scratch emulator build to select a mode for a standalone RPX. Games start
under the requested mode only when the startup log confirms it. For the
interpreter, the command-line disable marker takes precedence over Cemu's
reported configured CPU mode.

[The x64 carry patch](x64-subfe-carry-cemu-2.6.patch) makes the SBB paths of
PowerPC `subfe` write the inverse of x86's borrow to XER[CA]. The existing
ADC path already writes the PowerPC carry convention. Apply both optional
patches with `git apply --check` against the pinned source. These changes
belong to the host emulator build only.

The [PowerPC carry probe](../diagnostics/ppc_carry/README.md) on the existing
NSS build failed in multi-core mode at call 73,329. The branch control
and the forced interpreter passed. A scratch build with both optional
patches passed all one million calls in CPU modes 1 and 3. Its SHA-256
was `74ef9d30ec9f6de8c8adf5b2df64144b17bb656ded96cd7ac1be0547e3dbc6d5`.
The scratch emulator was linked from the existing NSS build inputs with
the two changed translation units compiled separately.

With the exact RC 20 RPX, the scratch multi-core emulator reached first
world entry and then trapped in a TCP_NODELAY call. The scratch single-core
run completed core resource preparation and returned to Night's startup
error. Both showed invalid Rust allocation counters before the world opened.
The passing instruction probe establishes one corrected translation and the
profile mode selection. Full recompiler gameplay remains unverified.

## Indexed reservation operands

Cemu 2.6's x64 emitter calculated the effective address for an indexed
`stwcx.` by adding the index to the native register that held the guest
base. Neither the successful store nor the address-mismatch path restored
the base. The next guest load could use the store address in place of its
original base. The ordinary indexed stores examined restore the base
after using it. Equal real address operands are handled by the existing
interpreter fallback.

The [reservation patch](x64-preserve-reservation-inputs-cemu-2.6.patch)
uses a scratch effective-address register for `lwarx` and saved EAX for
`stwcx.` so the indexed paths leave both guest operands intact. It keeps
the original compare-and-store and reservation conditions. Apply it
with `git apply --check` to pinned Cemu 2.6
`a6fb0a48eb437a8a41c13b782ac8ae0433bf8f98`. The earlier profile and carry
patches remain separate. Build in a disposable Cemu checkout.

A single-threaded [assembly fixture](../diagnostics/ppc_reservation/README.md)
reproduced the corruption on the carry-corrected build. After compilation
warmup, a successful indexed store made the following load from the base
return the value stored at base + 4. On a reservation address mismatch, the
following load read base + 128. Forced-interpreter mode passed both cases.
A separate Cafe Rust fixture reproduced incorrect peak and live-block
counters with a chain of relaxed atomics matching Horizon's measured
allocator. The diagnostic operations require no game allocations, world
data or GX2.

| Input | SHA-256 |
| --- | --- |
| Previous carry-corrected Cemu | `74ef9d30ec9f6de8c8adf5b2df64144b17bb656ded96cd7ac1be0547e3dbc6d5` |
| Reservation-corrected Cemu | `59c095297b50ca654ea014d7c950dbac2005e64754a101a1cb8184af7b409140` |
| Carry RPX | `8b4ff6bcfec63966817f50d8cd722d87b74f0266c9fdd122aacc9f67ba9305e4` |
| Reservation operand RPX | `c455416db37056bfdaec0fb741daffd0f03ac7941d8631e088ae6e17cfb474ad` |
| Rust allocator-counter RPX | `1d0d69fccb48838882339572c40b1ab5af17795e4b99e1cc161ff75a4b1c0c16` |

On the reservation-corrected build, each confirmed JIT mode passed
one million carry cases, one million reservation-operand iterations,
and 800,000 allocator-counter iterations. The repaired binary was linked
from the previous scratch Cemu build inputs with only its x64 emitter
translation unit recompiled. The checks validate targeted instructions.
The forced interpreter remains the fallback for hosted gameplay.

### RC 38 hosted gameplay in multi-core Cemu

On 26 November 2026, the reservation-corrected Cemu ran the exact RC 38 RPX
`2da87b74841f9c570a0efde9b7fd85577e1c5a0bc17c4b4fbc8010deab243553`
with the disposable offline Reopen Fixture. The earlier carry-corrected
build returned to Night's startup error on its first entry with the same
RPX and starting fixture. Its `hosted.before_game` counters reported
4,291,873,234 live and 4,294,965,399 peak Rust bytes. With the reservation
fix, those counters reported 16,496 live and 120,692 peak bytes.

First entry reached the original game's world-ready frame after 83.369 s.
The retained HZP3 session contained 82 playing records with one submission
to each output, with TV at 1920×1080 and GamePad at 854×480. Fixed 75%
world targets were 1440×809 and 641×360. The durable save preserved the
initial position. Movement was unverified. The run returned to Night
with 234,880,920 free GX2 bulk bytes and a largest free block of
234,880,896 bytes. Native audio
callbacks delivered nonzero PCM and the final queue read/write counts agreed.

The same-process reopen restored preparation caches and then the
`resource-loader` thread panicked in Rust's BTreeMap traversal
(`alloc/src/collections/btree/navigate.rs:164`). The runner stopped before
the second session could reach gameplay or verify the saved position
after reopening. Cemu saved byte-identical 854×480 PNGs under the TV and
GamePad filenames. With the same RC 38 RPX and Night settings, the previous
carry-corrected and reservation-corrected executables both reproduced the
duplicate in forced-interpreter mode. The established NSS interpreter,
SHA-256 `e80e74c64334bdbf27151aa2c248a3beac12f42cfdf3322f570f433dfb99d19d`,
produced a separate 1920×1080 TV PNG and 854×480 GamePad PNG. Use that
interpreter build for independent output captures and full lifecycle tests.
Authenticated multiplayer, repeated saves and reopens, and an in-app exit
remain unqualified on the reservation-corrected executable.

### RC 40 fresh entry and an iterator control

The reservation-corrected JIT returned to Night during fresh loading of
RC 40 RPX `e18b98834d09ae0af8359a9213f94adf5a35df5cca8b230c23d9001e68e56aa4`.
The failure appeared after about 26 s. The log closed the
`core.motion`, `core.light` and enclosing `core` timing spans and
showed no panic or allocator failure. The bulk heap recovered.
The same RPX and `4t` profile reached gameplay and saved in the established
forced interpreter. The JIT run ended before any gameplay terrain submission,
so it provides no measurement for the terrain-retirement trial.

A pinned Cafe Rust fixture allocated synthetic BTreeMaps with 0–24 tagged
string keys, checked immutable forward and reverse iteration and iterator
cloning, and verified collection into a BTreeSet and consuming map
iteration. RPX SHA-256
`4c5ec7f9fe823fa99a0029b869e93b0f060229479ee44302850a26b9e782ef40`
passed all 140,000 iterations in forced-interpreter, single-core JIT and
multi-core JIT modes. The synthetic key workload did not reproduce the
RC 38 warm panic. RC 40 reached a different observable boundary.
Result logging around core resource completion and publication of the
built-in registry orders can narrow the fresh-entry error.

## Disposable saved-world runs

Prepare a fresh lab from a disposable portable template and an offline world
with one player. Use the exact RPX recorded by the package receipt. Save and
report paths below are inside the lab.

```powershell
python -B tools/gx2_cemu.py $lab prepare --default-instance --capture `
    --cemu $cemu --rpx $rpx --template $template --game $preparedGame --scratch-world $fixture
python -B tools/gx2_cemu_acceptance.py $lab --cpu-mode interpreter `
    --sessions 3 --save-at 427 348 --save-timeout 500 --gameplay-dwell 120
```

Use a new lab for each CPU mode. `--startup-timeout` bounds slow world loads.
For the fatal-fall scenario, add `--respawn-fixture` and use an unused
Survival fixture. This raises the cloned player 96 blocks. D-pad Up and
physical A drive Respawn without pointer fallback, and the first save must
record the death location and restored health. Supply `--spawn-xz X Z`
when the fixture has a known respawn point to verify later movement. The
runner checks the reopened player's exact durable position, native PCM and
heap recovery on each round. The evidence includes both display captures,
the CPU mode and phase timestamps in `acceptance.jsonl`.

The local runner needs no account or network approval. For HTTPS checks,
use `tools/mcp_codex_cemu_test.py --resume` with the previously authorized
disposable identity and the same HTTPS endpoint. Add `--game` to repeat
the hosted launch and save using the persisted scopes and Night settings.
The resumed run leaves local permission controls alone and requires a new
save barrier and return to Night for each recorded round.
