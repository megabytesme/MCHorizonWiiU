# Nightly MCP comparison

The upstream Horizon pin `0a875da91dbb0fdcb7ba8c42c11341aa229af51a`
registers 194 tools. The assembled development tree
`aed79902d1582c03f91129b425591632e3cfc2dd` registers 56 stock tools through
Night's authenticated HTTPS endpoint. Night embeds the earlier Horizon
`69fab597a6f0e8a6ce524d38d1b73e599e343034` inventory and applies its
own availability and permission rules. The committed files hold the
upstream names, descriptions and schemas, and the exact source comparison
for each tool. [Upstream registrations](nightly-tools.json) · [Comparison](nightly-parity.json)

## Transport and permissions

| Surface | Discovery and transport | Authentication and authorization | Runtime behavior |
| --- | --- | --- | --- |
| Pinned upstream launcher and game | Stdio MCP via the launcher or `horizon-ctl`, with launcher-prefixed game registrations. Both dispatch tool discovery and calls. | The launcher runs in the user's local session. Game commands use the applicable game control transport and its existing restrictions. | The official snapshot includes launcher, dedicated-server, storage, mod, gameplay and benchmark commands. |
| Combined Cafe HTTPS | Modern server discovery and legacy initialize, scoped tool listing and calls. The same listener remains present across Night and compatible game sessions. | HTTPS, owner-approved DCR and PKCE, scoped OAuth tokens, refresh rotation, optional PIN and explicit revocation. RC 39 adds durable refresh trust. | A launcher grant cannot use game calls without the corresponding game scope and a live session. Launch also requires the local Allow agent game launch setting. |
| Cafe native encrypted LAN | Game-owned TCP control used by `horizon-ctl`. Stdio tool discovery runs on the PC side. | Per-activation pairing code and authenticated ChaCha20-Poly1305 sessions. Trust ends when Agent Control goes off or the native session exits. | Native diagnostics include controller state and GamePad capture. The actions exposed here do not automatically become available to the HTTPS principal. |

These paths expose tools rather than an MCP resources catalogue. Night's
HTTPS tools may return structured content, ordinary control replies,
tool errors or PNG content according to the adapter. Revocation and the
local emergency control remain available with the native HTTPS endpoint.
[HTTPS protocol and setup](README.md) · [Encrypted LAN commands](../../wiiu/game-bridge/agent-control.md)

## Differences to resolve

- `game_interact`, `game_place`, `play` and `settings` have upstream
  input schema differences. `game_break` and `game_goto` also have
  changed upstream descriptions. Check adapters and actual results before
  updating the advertised contracts.
- The HTTPS adapter exposes ordinary state, navigation, inventory,
  interaction, pause, save, quit and TV capture. Expose GamePad and paired
  captures and generic controller and touchscreen actions via the same
  HTTPS principal. RC 41 adds scoped graphics reads and writes. Keep screenshot readback on demand.
- Night can list instances, worlds and multiplayer bookmarks, select an
  instance, add a bookmark and start a compatible game. RC 42 adds scoped
  benchmark start, status, report access and cancellation. RC 43 adds
  existing profile selection and joining saved bookmarks. This source
  recipe adds cancellation before ordinary remote gameplay handoff.
  Disposable world creation, bookmark editing and clean application exit
  remain open.
- Exercise the live tool catalogue and outputs under all four granted
  scopes and after restart or upgrade. The exact-source comparison below
  checks registrations and embedded definitions. Runtime qualification
  must cover authorization, catalogue filtering and tool behavior.

## Reproduce the comparison

Use an exact upstream checkout or the existing object store with
`HORIZON_MCP_GIT_ROOT`. The second command takes the assembled Nightly
source directory produced by the builder.

~~~sh
python -B tools/mcp_inventory.py --revision 0a875da91dbb0fdcb7ba8c42c11341aa229af51a \
  --output docs/mcp/nightly-tools.json --check
python -B tools/mcp_parity.py --nightly-source "$assembled_nightly_source" --check
~~~
