# Cemu condition-register liveness regression

Run this host test with Python 3, Git, a GCC-compatible C++17 compiler, and a
local Cemu repository containing commit
`a6fb0a48eb437a8a41c13b782ac8ae0433bf8f98`.

~~~sh
python3 -B wiiu/cemu/cr-liveness-test/run.py /path/to/Cemu
~~~

Set `CXX` to choose another compiler executable. The test reads pinned Git
objects and applies [the patch](../ppc-cr-liveness-cemu-2.6.patch) to a
temporary copy. It compiles the original and patched IML linker and liveness
functions against small, explicit IML graphs. No emulator build or launch
is required. Generated source and host binaries stay in the temporary
directory.

The original optimizer suppresses CR0 updates before the graph containing
`mfcr` and an unknown call. It also suppresses CR0 before a known local
overwrite that follows an `mfcr` read. The patched optimizer preserves
both. Cases with a same-block read and a short call control remain valid.
The test also checks that a known overwrite with no intervening read still
eliminates dead updates. A local CR0 definition only supplies that field.
`mfcr` must still consume incoming CR1 through CR7.

The Cemu source in the patch and the temporary extracts is covered by the
[Mozilla Public License 2.0](../LICENSE-CEMU.txt). The host scaffolding is
covered by this repository's [MIT license](../../../LICENSE). The pinned
source itself is available from the [Cemu project](https://github.com/cemu-project/Cemu/tree/a6fb0a48eb437a8a41c13b782ac8ae0433bf8f98).

See [the recompiler results](../RECOMPILER.md#condition-register-liveness)
for the separate emulator regression.
