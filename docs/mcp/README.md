# Native Agent Control

Horizon Night owns the HTTPS endpoint in the combined RPX. Gameplay attaches
an in-process command queue to the same application. The HTTPS listener and
OAuth authorization stay available through resource preparation, gameplay,
save, and the return to Night. A new game gets a fresh queue generation.
The original authenticated LAN control remains available from the game's AI
Agent settings for diagnostics.

## Set up on Wii U

1. Open **Agent Control** in Horizon Night. Choose a local PIN or confirm
   **Use no PIN**. A copied SD card can expose unencrypted secrets in no-PIN
   mode. PIN mode requires local entry after a cold start. Night shows
   whether the vault found older copies it could not retire.
2. Open **HTTPS connection**. Note the URL and both halves of the SHA-256
   fingerprint. Obtain the public **launcher/agent-control-ca.pem** from the
   application's SD directory. Compare the certificate's SHA-256 fingerprint
   with the GamePad before trusting it. Keep the **private/** directory on
   the console.
3. Trust that public CA in the desktop browser's OS or browser trust store.
   The browser needs it to complete the OAuth redirect flow. For Codex CLI
   you can also set **CODEX_CA_CERTIFICATE** to the PEM path. For Gemini CLI
   set **NODE_EXTRA_CA_CERTS** to the PEM path before starting the CLI.
4. Choose **Pair a PC** on Wii U just before initiating the first OAuth
   login. In your browser choose **Request approval**. Review the matching
   pairing ID, client name, callback and scopes on your GamePad. Approve
   only the request you initiated. You can revoke a named client at any time.
5. Enable **Allow game control** to expose game tools while a game is live.
   **Allow agent game launch** adds permission to start a controllable
   session from Night. The client also needs the corresponding OAuth scopes.

Compare the fingerprint with **openssl x509 -in agent-control-ca.pem -noout -fingerprint -sha256**.
On Windows, **certutil -decode agent-control-ca.pem agent-control-ca.cer**
converts the PEM to DER. Use **certutil -hashfile agent-control-ca.cer SHA256**
to compare its fingerprint.
Trust the CA you copied from the console and compare its fingerprint before import.

### Codex CLI

The tested Codex build rejects OAuth discovery when the authorization server
uses a private IPv4 literal. The same error occurred against the Cemu HTTPS
endpoint after both discovery documents passed certificate validation.
The combined RPX passed direct Codex OAuth and launcher calls in NSS-patched
Cemu 2.6 using a resolvable `.local` hostname and the exported CA.
[Codex issue 55163](https://github.com/openai/codex/issues/55163)

Configure a [LAN hostname](#lan-identity) before
pairing. Use the displayed HTTPS URL, including its port and **/mcp**. Keep
the same **horizon-wiiu** server name and use your regular Codex profile so
later sessions can reuse the pairing. For an owner-approved automation PC,
request all four scopes during the initial login.

~~~powershell
$env:CODEX_CA_CERTIFICATE = 'C:\path\to\agent-control-ca.pem'
codex mcp add horizon-wiiu --url https://horizon-wiiu.home.arpa:8443/mcp
codex mcp login horizon-wiiu --scopes launcher.read,launcher.manage,game.observe,game.control
~~~

The launcher scopes are **launcher.read** and **launcher.manage**. Game
scopes are **game.observe** and **game.control**. A launch needs
**launcher.manage** and **game.control**. Night asks once for the scopes
present in the OAuth transaction. To pair a read-only launcher client,
request just **launcher.read**. [Codex MCP configuration](https://developers.openai.com/codex/mcp)

### Gemini CLI

Set **NODE_EXTRA_CA_CERTS** and configure a Streamable HTTP endpoint in
**settings.json**. The OAuth option explicitly permits discovery and token
exchange through this private network.

~~~json
{
  "mcpServers": {
    "horizon-wiiu": {
      "httpUrl": "https://192.168.1.25:8443/mcp",
      "oauth": {
        "enabled": true,
        "allowPrivateNetwork": true,
        "scopes": ["launcher.read", "launcher.manage", "game.observe", "game.control"]
      }
    }
  }
}
~~~

Use **/mcp auth horizon-wiiu** if Gemini prompts for authentication.
The normal loopback browser callback is part of the client.
[Gemini CLI MCP configuration](https://geminicli.com/docs/tools/mcp-server/)

## LAN identity

Night binds to the console's assigned private IPv4 address on port 8443.
An IP certificate works directly. A DHCP reservation can keep that URL
stable. To use LAN DNS, put a lowercase **.home.arpa** or **.local** name
in **launcher/agent-control-hostname.txt** before first setting up Agent
Control. Configure LAN DNS or your host resolver for that name. Night
uses the hostname as the OAuth resource identity, so it can survive a
LAN address change if clients continue using the same hostname. Night
does not advertise the hostname with mDNS.

Rotate from **HTTPS identity** when replacing the canonical hostname or
IP. Rotation invalidates the paired clients. Remove the old public CA
from trust stores and verify the new fingerprint before installing it.
The endpoint is for clients on the LAN. A cloud client cannot use the
private address directly.

## Protocol and authorization

The **2026-07-28** path uses per-request metadata and routing headers.
It supports **server/discover**, **tools/list**, and **tools/call** with
JSON responses and private, non-cacheable tool lists. The **2025-06-18**
path uses the legacy **initialize** exchange and omits protocol sessions.
There is no standalone SSE stream. The server advertises tools without
the optional tool-list change notification capability. Refresh the tools
list when a game starts or stops if your client caches the legacy list.

Night uses Rustls with the existing Cafe ring and system entropy integration.
The leaf key, its certificate, and the public CA persist through the shared
MCP vault domain. The one-time CA signing key is discarded after issuance.
OAuth uses resource and authorization server metadata, public native client
DCR, exact loopback redirects, approval on the GamePad, PKCE S256, scoped
access tokens, refresh rotation and replay detection. The vault holds token
verifiers and the refresh-family authentication key. It contains no
plaintext OAuth bearer tokens.

In the hosted development recipe, approved clients that support refresh keep
their trust across restarts and inactivity. Their credential manager
automatically exchanges the saved refresh credential for a new access token,
which lasts 15 minutes. The pinned 0.7.0 release issued refresh grants with a
90-day expiry. An existing grant becomes durable on its first successful
refresh in the updated build before the old deadline. An already expired grant
or a client that lost its refresh credential needs local approval. The
trusted-client menu can revoke the grant at any time.

Use **No AI / Emergency disable** locally to cancel access and game work.
Credentials and launch permissions cannot be changed using remote tools.
Night returns a tool error when a compatible live game session is absent.
Local confirmation for irreversible actions is reserved for reviewed
adapters. The current service does not advertise commands that delete worlds,
change arbitrary security settings, or start dedicated servers.
[Tool inventory and dispositions](parity.md)

### Live graphics in the hosted development recipe

Use `game_options_get` with `game.observe` to read game options. For example,
pass `{"keys":"horizonGraphicsPreset resolutionScale dynamicResolution targetFps"}`.
`resolutionScale.value` is the saved configuration, `requested` is the scale
for the next frame, and `last_paired_submission` reports the requested and
effective scale of the last accepted TV and GamePad pair with its age.

Use `game_options_set` with `game.control` to apply one option by key and value.
`{"key":"horizonGraphicsPreset","value":"full_performance"}` applies the same
stock settings as the graphics screen. `{"key":"resolutionScale","value":"75"}`
sets the configured scale. With DRS off, 50 through 100 are accepted in 5-point
steps. `dynamicResolution` and `targetFps` follow the native trial gate. The tool
also accepts the stock `renderDistance`, `simulationDistance`,
`entityDistanceScaling`, `renderClouds`, `particles`, `entityShadows`,
`mipmapLevels`, `biomeBlendRadius`, `weatherRadius` and `gamma` keys. Read back
the last paired submission before opening a fixed-scale measurement window.
Graphics writes are unavailable during a running benchmark.

### Existing profiles in the hosted development recipe

Use `wiiu_profiles_list` with `launcher.read` to get existing profile IDs,
display labels and readiness. From the Night home page, use
`wiiu_profile_select` with `launcher.manage` and the returned `id`.
When an online selection reports `refreshing`, poll `wiiu_profiles_list`
until the profile is selected and ready before starting a launch. Locked
profiles use Night's local unlock flow. World listing continues to report
when a save belongs to a different profile.

### Saved multiplayer joins in the hosted development recipe

Use `multiplayer_list` to get the canonical address of a saved server.
Pass that address to `wiiu_multiplayer_join` from the Night home page.
The tool requires `launcher.manage`, `game.control` and Night's remote
launch policy. It uses the selected profile and the ordinary connection
path. Add an entry through `multiplayer_add` when the server is missing
from the list.

### Remote Night cancellation and close

Ordinary remote `play` and `wiiu_multiplayer_join` responses include a
`launch_id`. The owning client can use `wiiu_loading_status` with
`launcher.read` and `wiiu_loading_cancel` with `launcher.manage`,
`game.control` and Night's remote launch policy. Cancellation completes
after the worker drain and is available before gameplay handoff.

From idle Night Home, use `wiiu_close_to_menu` with `launcher.manage`.
The ordinary Close Horizon action requires any previous launch, profile
selection or save-to-Night transition to have finished.

### Benchmarks in the hosted development recipe

With `launcher.manage` and `game.control`, call `wiiu_benchmark_start` from
the Night home page with a scenario such as `{"scenario":"cafe-stationary"}`
or `{"scenario":"village"}`. Night's remote launch and game control settings
must be enabled. Each scenario uses the same isolated workspace, settings
and route as when started locally.

Call `wiiu_benchmark_status` with `launcher.read` to follow the queued,
loading, running and closing phases. A completed run returns its `instance` and `report_id`.
Call `wiiu_benchmark_cancel` with the same permissions as start to cancel
a queued or active benchmark. Wait for the `cancelled` phase before starting
another run.

With `launcher.read`, call `wiiu_benchmark_reports` to list saved report IDs
in the selected installation or pass `{"instance":"Lab"}` to list an
explicit installation. To read a completed run after switching
installations, pass its instance and ID to `wiiu_benchmark_report`, for
example `{"instance":"Lab","id":"game-20261126142051-village.json"}`.
The JSON includes the benchmark conditions, metrics and frame
distributions. Saved reports remain available after restarting Night.
The live status starts in `idle` after a restart. Local labels and
attachment paths stay on the console.

### Gameplay captures

`game_screenshot` returns the TV PNG during live gameplay. A scratch Cemu
build with `HORIZON_WIIU_CEMU_GLOBAL_SESSION_LOCK=ON` also advertises
`game_screenshot_gamepad`. Both require `game.observe`. For a pair, call the
TV tool and wait for its reply before calling the GamePad tool. Each reply
contains one PNG and JSON with its `display`, `width` and `height`.

Physical GamePad capture needs bounded staging and qualified GPU retirement
and cleanup. The RC 26 physical probe reported a fatal readback cleanup error.
Keep screenshot requests outside timed gameplay windows.
[RC 26 physical evidence](../gx2/physical-rc26.md)

## Build and test

The pinned release combination is client **0.7.0**, patch **0.4.0** and
toolchain **1.0.0**. For durable trust, build the hosted Nightly recipe
starting with **0.8.0-rc.39** and Cafe Toolchain **1.1.0**. The HTTPS service is part of
the normal combined RPX, and the builder verifies its exact source and lock.
The standalone launcher and game use their original entry points.
[Builder guide](../builder.md)

[Unified application validation](../releases/client-v0.7.0.md) ·
[Original MCP feature validation](validation.md)
