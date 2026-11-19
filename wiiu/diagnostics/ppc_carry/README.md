# PowerPC carry diagnostic

This development probe compares a PowerPC carry-dependent ordering sequence with
an independent branch-based control over one million calls and eight unsigned
input pairs. It needs no game resources or SD card. A passing run reports
`chain_first_bad=0` and `branch_first_bad=0` to OSReport and exits with code 0.
Nonzero fields give the first failing call; `value` is the first chain result.

With devkitPro, devkitPPC, WUT, and elf2rpl installed, build into a disposable
directory with `DEVKITPRO=/opt/devkitpro DEVKITPPC=/opt/devkitpro/devkitPPC
bash wiiu/diagnostics/ppc_carry/build.sh /tmp/horizon-ppc-carry`. The script
refuses to overwrite either output file; choose a fresh directory for each build.
Run the RPX in Cemu and inspect its log. Check the active CPU mode and whether the recompiler
initialized; use the same RPX for each comparison.

On the tested stock Cemu 2.6 build, the interpreter (profile CPU mode 0) passed;
recompiler profile modes 1 and 3 returned an incorrect chain result after
warmup while the branch control passed. The failing call count varies. These
are observations for that emulator build, not a claim about every Cemu release
or physical Wii U hardware. This diagnostic does not fix the emulator or prove
that the game is otherwise compatible.
