# Experimental Cemu NSS support

This is an **emulator-only diagnostic patch**, not part of Horizon or the Wii U
RPX. It adds Cemu HLE exports for `NSSInit` and `NSSGetRandom`, backed by
OpenSSL `RAND_bytes`; stock Cemu 2.6 returns success without writing random
bytes for this title. Never use a fixed or simulated random source for this.

The patch targets Cemu v2.6 commit
`a6fb0a48eb437a8a41c13b782ac8ae0433bf8f98`, from
[the Cemu project](https://github.com/cemu-project/Cemu). Cemu's source and
this modified Cemu source file are under the [Mozilla Public License 2.0](https://github.com/cemu-project/Cemu/blob/v2.6/LICENSE.txt).
The zero-context patch was checked with `git apply --unidiff-zero --check`
against the unmodified file from that exact commit. Verify the commit before
applying it: without context, a different source revision is not safe to
patch. It must not be applied to the user's installed Cemu.

The code is experimental: `NSSFinish` and `NSSLibReady` are not implemented;
real-console initialization/lifecycle semantics have not been compared; the
generic OpenSSL failure result has not been verified against Cafe; and guest
buffers spanning separate Cemu mappings may be rejected. `RAND_bytes` failure
returns a nonzero error. A passing buffer smoke test is not a cryptographic
audit or real-hardware verification. `getrandom` 0.3/0.4 integration remains
separate and is not solved by this patch.

The separate [`crnand-cemu-2.6.patch`](crnand-cemu-2.6.patch) adds the missing
PowerPC `crnand` interpreter instruction for the same Cemu revision. Cemu's
recompiler falls back to the interpreter for the containing function. Check the
revision and use `git apply --unidiff-zero --check` before applying it with
`git apply --unidiff-zero` alongside the NSS patch. Do not apply either patch
to an installed emulator. The title diagnostic's
result and its remaining limitations are recorded in `PORTING.md`.

## Isolated build performed

The source was cloned to `E:\dev\HorizonWiiU-cemu-nss-20261107\source`,
with a separate `build`, temp directory, vcpkg downloads, binary cache and
triplet overlays under the same E: directory. The installed emulator was not
modified. Choose a new, empty task directory to recreate the checkout and patch
from a PowerShell prompt:

```powershell
$taskRoot = 'E:\dev\HorizonWiiU-cemu-nss-20261107'
New-Item -ItemType Directory -Force -Path "$taskRoot\temp" | Out-Null
$env:TEMP = "$taskRoot\temp"
$env:TMP = $env:TEMP
git clone --depth 1 --branch v2.6 https://github.com/cemu-project/Cemu.git "$taskRoot\source"
git -C "$taskRoot\source" submodule update --init --recursive --depth 1 --jobs 4
if ((git -C "$taskRoot\source" rev-parse HEAD) -ne 'a6fb0a48eb437a8a41c13b782ac8ae0433bf8f98') { throw 'Unexpected Cemu revision' }
git -C "$taskRoot\source" apply --unidiff-zero --check "C:\dev\personal\HorizonWiiU\wiiu\cemu\nlibnss-cemu-2.6.patch"
git -C "$taskRoot\source" apply --unidiff-zero "C:\dev\personal\HorizonWiiU\wiiu\cemu\nlibnss-cemu-2.6.patch"
```

Build prerequisites are Cemu's pinned vcpkg submodules, the Windows C++
toolchain and SDK, Ninja, CMake, and network access. The actual reproducible
build used the preserved `$taskRoot\configure.cmd` followed by
`$taskRoot\build-cemu.cmd`. These scripts use VS 18 Community's `vcvars64.bat`
and CMake/Ninja, direct TEMP, TMP, `VCPKG_DOWNLOADS`, and the vcpkg binary cache
to E:, and build target `CemuBin`. To reproduce on a machine with a normally
registered supported Visual Studio installation, follow Cemu's `BUILD.md`
after applying the patch; the following local overrides were needed here:

- The unregistered VS instance required local x64 Windows/static triplet
  overlays using the initialized compiler environment and
  `VCPKG_LOAD_VCVARS_ENV OFF`.
- Pinned vcpkg requested a removed MSYS2 pkgconf archive. An existing local
  pkg-config 2.5.1 was copied to `$taskRoot\pkgconf` and verified against the
  generated OpenSSL metadata; its executable SHA-256 was
  `BDDC890337D8A76305E1A644884AE6995C452C0AD7781EF2AD729E41B5DE8AA5`.
  `PKG_CONFIG` and `VCPKG_KEEP_ENV_VARS` passed it to vcpkg without changing
  system tools.
- CMake 4 required `CMAKE_POLICY_VERSION_MINIMUM=3.5` for an older dependency;
  that variable was also passed through vcpkg. A local copy of the pinned
  libusb port used the installed `v145` toolset for its MSBuild project rather
  than vcpkg's `external` label. Both overrides are limited to this checkout.

## Results

The patched build produced `source\bin\Cemu_release.exe`, SHA-256
`05373C525BB660620CF1E21884C7732E5A16810BD8A0739B8190B61701223C6D`.
Two separate isolated portable Cemu directories were created under `$taskRoot`;
each had its own SD and MLC. The existing `wiiu/entropy_probe` RPX (SHA-256
`443F6CD1FB1B7DA459C1202700BFBE692CA2307ABE0B7850C025933D83073A6E`,
Cemu checksum `0x5966ba5e`) reached title and wrote `failure_flags=0`, after
stock Cemu had written `failure_flags=15`. The guest called
`coreinit.exit(0)`; Cemu's GUI process stayed open and the test stopped its
own process after 35 seconds. This excludes the observed unchanged-buffer
failure; it does not establish entropy quality.

A separate diagnostic RPX (SHA-256
`A10E5A1FC434F0E7D2934BA72EEF79AD55C092DF07CBBFEBDFC30B55C4578211`,
Cemu checksum `0x742a3174`) also wrote `failure_flags=0` and called
`coreinit.exit(0)`. It checked expected pre-init, null-pointer, negative-size,
and unmapped-buffer results plus the same buffer smoke tests. Its own Cemu
process was stopped after 20 seconds. The diagnostic source is
`nss_abi_probe.rs`; to rebuild it, copy `wiiu/entropy_probe` and
`wiiu/bootstrap/prepare_wut_startup.sh` into adjacent directories in a scratch
location, replace the copied `entropy_probe/src/lib.rs` with this file, and
run the copied `entropy_probe/build.sh` with the Cafe stage-1 compiler as
shown in `wiiu/entropy_probe/README.md`. Flag bits 1, 2, 4, 8 and 16 represent
pre-init, null, initialization, negative-size and unmapped-buffer failures;
bits 32, 64, 128 and 256 represent unchanged/identical output checks. Read the
status file rather than relying only on the low bits of a process exit code.

Logs, RPXs, status files and the actual executable are preserved at
`E:\dev\HorizonWiiU-cemu-nss-20261107\run-std` and `run-abi`. They are
local test artifacts, not a menu or a playable client.
