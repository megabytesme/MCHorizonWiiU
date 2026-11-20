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
endpoint after both discovery documents passed certificate validation. Use
Gemini for the IP endpoint. Codex with a configured LAN hostname needs its
own interoperability test. [Codex issue 55163](https://github.com/openai/codex/issues/55163)

Use the displayed HTTPS URL, including its port and **/mcp**. To request
a read-only grant initially, start manually with
**codex mcp login horizon-wiiu --scopes launcher.read** after adding the
server, or choose scopes during your client's supported login setup.

~~~powershell
$env:CODEX_CA_CERTIFICATE = 'C:\path\to\agent-control-ca.pem'
codex mcp add horizon-wiiu --url https://192.168.1.25:8443/mcp
~~~

The launcher scopes are **launcher.read** and **launcher.manage**. Game
scopes are **game.observe** and **game.control**. A launch needs
**launcher.manage** and **game.control**. Night asks once for the scopes
present in the OAuth transaction. The client's credential manager handles
access-token expiry and refresh. [Codex MCP configuration](https://developers.openai.com/codex/mcp)

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

Use **No AI / Emergency disable** locally to cancel access and game work.
Credentials and launch permissions cannot be changed using remote tools.
Night returns a tool error when a compatible live game session is absent.
Local confirmation for irreversible actions is reserved for reviewed
adapters. The current service does not advertise commands that delete worlds,
change arbitrary security settings, or start dedicated servers.
[Tool inventory and dispositions](parity.md)

## Build and test

Use the canonical builder's development combination for client **0.7.0**,
patch **0.4.0** and toolchain **1.0.0**. The HTTPS service is part of
the normal combined RPX, and the builder verifies its exact source and lock.
The standalone launcher and game use their original entry points.
[Builder guide](../builder.md)

[Unified application validation](../releases/client-v0.7.0.md) ·
[Original MCP feature validation](validation.md)
