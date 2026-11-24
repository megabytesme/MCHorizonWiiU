# Standalone game profiles and x64 carry diagnostic

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

The local runner needs no account or network approval. For HTTPS checks, use
the existing `tools/mcp_codex_cemu_test.py --resume` flow with a previously
authorized disposable identity, or arrange the normal consent separately.
