# Launcher core and presentation

Horizon Night is the built-in presentation for this launcher. It renders
independent layouts for the TV and GamePad from the same value snapshot,
`launcher::View`. Each page has typed actions, focus, availability, and
selection. The GamePad hit test returns action IDs. The core checks page
membership and action availability before dispatching each press or tap.

The Cafe host owns one ProcUI pump, WHB GX2 initialization, and the display
targets. The host borrows `Launcher::live_driver` for a foreground gameplay
frame. Launcher navigation receives input during shell frames. The real
adapter will receive typed native game input during its live frames. On
foreground loss, the session releases foreground-only resources before the
host acknowledges release. The next acquire uses fresh targets and neutral
input evidence. The prototype renderer is also used for the fixture's live
page.

## Session boundary

`SessionFactory::start` receives a snapshot of the selected profile,
instance, target, prepared-resource descriptor, and settings. It creates a
single `SessionDriver` that owns its startup work and any partial resources.
`HostedSession` drives the following transitions.

| Phase | Completion or request | Next phase |
| --- | --- | --- |
| Preparing | Backend reports ready | Live |
| Preparing | Cancel or startup error | Stop, drain, drop, then show Play or the startup error |
| Live | Host action or game quit request | Saving |
| Saving | Save or disconnect confirmed | Stop, drain, drop, then show Play |
| Saving | Backend reports failure | Save failed, with retry available |
| Save failed | Retry | Saving |
| Any active phase | CafeOS exit | Phase-specific exit barrier, drop, then host shutdown |

Preparation progress comes from the driver. A missing percentage yields an
indeterminate indicator. Saving and stopping lock the action while work is
pending. A save failure retains the same session and its world lock. The OS
exit path reports any terminal save error and releases the session before
destroying the renderer.

The build ships a mock driver that acquires an allocation during preparation,
delays save completion and cleanup independently, and exercises the partial
startup failure pathway. It uses synthetic data. The mock's timed phases
serve as a Cemu fixture. Host tests also exercise save failure and retry, the
four OS exit modes, and a scratch save written and synced by a worker before
the session lease is released.

See [Integration requirements](INTEGRATION.md) for the game bridge and
[builder instructions](../builder/README.md) for the pinned build.
