# Stock HUD warm reopen in Cemu

An opt-in RC 47 trial compared two ordinary entries of the same scratch
save in one interpreter process. Both used Full Performance at fixed
75%, DRS off, render distance 6 and simulation distance 5. Both captured
separate 1920×1080 TV and 854×480 GamePad outputs. The first observed
paired terrain sample in each round had one section on each output.

| Milestone or retained state | Cold | Warm reopen |
| --- | ---: | ---: |
| First ready Game frame | 696.436 s | 529.072 s |
| Verified HUD preparation | 62.592 s | 3.413 s |
| Fresh archive digest | 56.266 s | 51.528 s |
| Ordinary descriptor digest | 56.270 s | 0 s |
| Retained seed capacity | 10,351,352 B | 10,351,352 B |
| GX2 bulk free after return | 234,880,920 B | 234,880,920 B |
| Largest free GX2 bulk block | 234,880,896 B | 234,880,896 B |

The first ready game frame arrived 167.364 seconds earlier on reopen
in this interpreter run. The warm round mapped the HUD after the core
load, presented it and avoided the cold fallback. Both rounds completed
the save barrier and returned to Night. The process exited normally
after the second round. Other preparation caches also warm during the
first round. This comparison does not isolate the HUD seed's contribution
to whole-startup time.

The stock HUD trial used Cafe Toolchain 1.1.0, assembled engine tree
`38396809d676ba527c6f01f6aae6eb0e729eed51` and RPX SHA-256
`71260d23ca866f630969a72772e548911cd2d0f09170cd0d37184e775a49b3b0`.
The first attempt reached a save barrier after a delayed reply and
stopped before an observed return to Night. The complete replay used
the same package with separate scratch inputs. The physical acceptance
target remains sub-minute entry with faster reopen.
