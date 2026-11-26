# PowerPC reservation address operands

The fixture verifies that indexed `lwarx` and `stwcx.` preserve their input
registers. It uses only C, PowerPC assembly and WUT. It opens no game saves
and does not need a prepared Minecraft archive.

~~~sh
bash wiiu/diagnostics/ppc_reservation/build.sh "$scratch"
~~~

Launch the same RPX in fresh Cemu labs with CPU modes interpreter, single,
and multi. Check the mode in Cemu's startup log. An unpatched standalone
launcher may not honor the selected profile. Use the [standalone profile
patch](../../cemu/standalone-profile-cemu-2.6.patch) for the JIT
comparisons.

~~~powershell
python -B tools/gx2_cemu.py $lab prepare --cemu $cemu --rpx $rpx --template $template
python -B tools/gx2_cemu.py $lab start --cpu-mode $mode
python -B tools/gx2_cemu.py $lab wait "ALLOC_COUNTER result="
python -B tools/gx2_cemu.py $lab stop
~~~

A successful run logs `ALLOC_COUNTER result=PASS iterations=1000000`.
The first failure in each check includes its iteration, actual and
expected values. The checks cover the following cases.

| Mode | Assertion |
| --- | --- |
| 0 | A successful indexed store preserves the base used by the following load. |
| 1 | The successful store writes the value at base + 4. |
| 2 | A failed reservation address match preserves the base. The mismatch targets base + 128. |
| 3 | The load leaves a shared base and index register intact. |
| 4 | The store leaves a shared base and index register intact. |
| 5 | The store with a shared base and index writes the expected value. |

The important comparison is subsequent guest register behavior. A store
that writes to the correct address can still corrupt the base for the next
instruction. Cemu 2.6 falls back to the interpreter when the emitted base
and index use the same host register. Modes 3–5 also cover that fallback.
