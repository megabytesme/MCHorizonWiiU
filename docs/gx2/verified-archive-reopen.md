# One verified prepared archive across a launch

The trial pins one freshly verified archive descriptor before pack resolution.
The ordinary resource path and the stock HUD seed read through it. Each launch
still verifies the installed archive SHA-256. Registration, cancellation and
pack restart use the existing load lifetime. The host fixture checked that
atomic replacement cannot change the pinned bytes and that unpinning restores
an ordinary open.

## Exact trial package

| Input or output | Identity |
| --- | --- |
| Engine tree | `d34bbfc72083d0d0708ae3354f151b6610481de3` |
| Recipe SHA-256 | `2f1c850bd403d0bad7b491fb9d100bd818745cf784f1ac8d189fbdf883a03274` |
| RPX SHA-256 | `77040b74ef00b24f04c8b962e96ae16714aef19bc3d7be7f919369e1f9bb6335` |
| WUHB SHA-256 | `bfdff77a0da0d8173c03bb458e48bd4de4ea0f76afe225aa271f332ff3e8e28c` |
| Estimated Aroma code-heap headroom | 12,613,696 bytes |

Ben-Desktop verified this branch with Cafe Toolchain 1.1.0 in
`/var/tmp/mchorizon-verified-archive-handoff.d3EEwnah/out`. It includes the
experimental mapped stock HUD seed and warm replay branch. Both seed markers
and an exact installed resource receipt are required to opt in.

## Same-process Cemu replay

| Measurement | Cold | Warm |
| --- | ---: | ---: |
| Fresh archive verification | 51.467 s | 46.237 s |
| HUD seed preparation | 60.013 s | 3.604 s |
| First ready gameplay frame | 616.214 s | 535.448 s |
| First paired terrain, wall time since Play | 645.587 s | 561.680 s |
| Retained seed capacity | 10,351,352 bytes | 10,351,352 bytes |
| GX2 bulk heap free after save and drop | 234,880,920 bytes | 234,880,920 bytes |
| Largest bulk heap block after drop | 234,880,896 bytes | 234,880,896 bytes |

The offline interpreter captured distinct 1920×1080 TV and 854×480 GamePad
images at fixed 75%, render 6 and simulation 5. Each first terrain sample had
one section per output. The warm launch used the mapped HUD after core loading.
Both rounds saved the exact original position and rotation, returned to
Night, and the title then exited normally.

The warm round reached gameplay 80.766 seconds sooner. This is a same-process
comparison with changed cache state. The host descriptor tests establish that
the ordinary and HUD paths share the verified input. The runtime replay shows
that the integrated branch can load, save and reopen the world. Physical
loading times and isolated A/B attribution remain open.
[Earlier warm HUD replay](warm-hud-replay.md)
