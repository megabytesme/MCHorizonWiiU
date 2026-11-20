# Horizon Night local features validation

This record covers the isolated `codex/wiiu-night-local` worktree. The
release package uses `6a0244eebbb7a4c910ecad5916395e1ba5c4b187`.
The integrated comparison baseline remains
`2ba7af8fad6534a5a72b48e2a1d3ba993049dc12` with its verified packages.

## Native build

| Item | Verified value |
| --- | --- |
| Hosted builder | `0.4.0` |
| Hosted client | `0.6.0` |
| Hosted patch release | `0.3.0` |
| Catalog schema | `2` |
| Horizon revision | `69fab597a6f0e8a6ce524d38d1b73e599e343034` |
| Stable Wii U game baseline | `1a0a2948c33d11744eb403247cb566b0aa3ee858` |
| Hosted source tree | `14592e87cc28b392b2a19a6f159766352ad46a0c` |
| Hosted recipe SHA-256 | `ccf5e2c403bae5e32ae0919f0cc0501d2066e42f1166ae3db188b36a445f957b` |
| About build identity | `1f6c909df6ceaa6e18ff52e2754642ac7f595594b081f782eb6812f68a5f3bb4` |
| RPX SHA-256 | `17a553866a6a972dac2204da22512a86d57c404a452d707aa586982de8a15a22` |
| WUHB SHA-256 | `f61fe89a01401cb05159158a2025bd169c60ed1644cbab20441b71ebef0405f6` |

`build-hosted` verified the stable assembly and staged the new catalog and
patches in a private RAM-backed Linux mount. Cargo compiled offline and
locked with the pinned Cafe sysroot. The builder verified imports, unique
lifecycle symbols, and package hashes. Two independent clean builds used
the same compiler-visible mount path and produced byte-identical packages.
The standalone Night and diagnostic Night fixture rebuilt and verified.
The previous standalone game recipe and output remain the baseline.

Release packages and records are under `$scratch/HorizonWiiU-night-local/release`.
The repeat run is under `$scratch/HorizonWiiU-night-local/release-repeat`.
Follow the [builder instructions](../wiiu/builder/README.md) for a fresh build.

## Host and Cemu

Twenty-one Rust host tests and nine builder tests passed. Rust formatting,
Clippy and the public-tree check passed. The host cases cover interrupted
replacement, version migration, failed writes, offline identity stability,
compatible resource selection, read-only world enumeration, lock and missing
world rechecks, endpoint validation, settings handoff and report viewing.

All emulator tests used isolated portable profiles and the NSS-patched
Cemu 2.6 interpreter. Emulator SHA-256
`e80e74c64334bdbf27151aa2c248a3beac12f42cfdf3322f570f433dfb99d19d`.
The test used the previously prepared private 26.3 cache. The resource
manifest SHA-256 was `fdeb3599cd32d31daaaffd3ee06f834a6e8b4e2651f15c8d701519cec4b93a58`.
Each fixture has separate writable metadata, preferences and saves.
Prepared data directories were reused. The benchmark viewer read a labeled
synthetic QA report so the displayed value can be independently checked.

| Scope | Cemu observation |
| --- | --- |
| Profile and installations | Created and renamed a profile while the login identity stayed fixed. Night distinguished missing and corrupt installations and selected the compatible alternate `Lab` instance. |
| UI actions | Observed profile selection, A/B and X/Y changes, disabled Night touch, server add and selection, report reading and both About pages. The About digest matched its package build record. |
| Exact release RPX | Opened the system keyboard, dismissed it with Minus, created an identity using the character picker, and closed with `coreinit.exit(0)`. Restarted the same fixture and observed the selected profile and swapped buttons. Corrupting its newest configuration exercised fallback to the previous generation. |
| Native handoff | The preceding package from `9e3b5b826660ec889df593362c3b4584a142e864` (`e547c3cf2708409d1c54b9e63833996daa60cea11117ff95fe958ba3fd740000`) reached the native title and returned to Night in the same process. The first feature package also confirmed swapped B accepted in the native title. |
| Direct world | That same preceding package blocked a copied save under a mismatched identity. With the original offline identity it passed the saved directory ID to the native loader, started the integrated server, and reached playable terrain at render distance 12. Swapped X opened the original inventory screen. |
| Save and teardown | The direct-world run logged `hosted save barrier completed`, the game and loading worker release, and Night's restoration, in that order. Independent reads decoded 315 occupied region records across four files and checked the metadata files. Closing Night reached guest `coreinit.exit(0)`. |

The complete save trace and TV and GamePad captures are under the local
`cemu-final` fixture. The release-only restart and recovery evidence is
under `cemu-release`. The final package preserves the same world, save and
multiplayer bridge as the direct-world test. Its last changes fix standalone
linking and add a reliable keyboard dismiss action. This test covers a
copied save in Cemu. Physical saved-world reopening remains an independent
investigation. Live server connection, physical text keyboard input and
physical FAT32 recovery still need acceptance.

See the [feature action matrix](launcher-features.md) for storage and
unsupported features. Microsoft sign-in, entitlement checks, secure refresh
storage and launcher-owned Agent Control pairing remain pending.
Night's GX2 font atlas substitutes `?` for unsupported characters in labels.
The stored labels and world identifiers retain their original UTF-8.
