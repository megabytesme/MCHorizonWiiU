# Native MCP validation

The port worktree starts at **cf2ac8cbbad809a878e59aa4b20288a551294283** on
branch **codex/wiiu-native-http-mcp**. The separately owned security
implementation was imported from **bd9ee102005efd90cb7142ec55048127a96f60b6**
and **5f2e34b0b56e16cb0ca22f4e3ed6cf9a190ffc3a**. The packaged application
uses source commit **aacb5f9a15464b4d942bcc5a7e92fe3605b4808d**.

## Packages

| File | SHA-256 |
| --- | --- |
| HorizonWiiU.rpx | 0110e99baee7486227398c9fb71ccc1bcf2aee3df46665afdb47d4ff883b0f9e |
| HorizonWiiU.wuhb | b510d5f1258c38a1d083b40af303a3e3a214b8d9f9ec4b60d18f9caf080f204d |

The native OAuth feature package was built on Ben-Desktop in Ubuntu WSL
using the feature branch's hosted builder. The build receipt and independent
acceptance records were retained with that feature's packages. The
[unified release record](../releases/client-v0.7.0.md) tracks the canonical
builder and subsequent tests by the combined application's own RPX hash.

## Host fixtures

- Eleven MCP Rust tests cover the old and modern protocol paths, routing
  headers, access scopes, TLS trust and hostname mismatch, PKCE and
  callback binding, refresh rotation and replay, multiple clients, PIN
  and automatic-unlock storage, malformed or interrupted HTTP/TLS and
  game-request cancellation.
- Twenty-one pregame tests and nine existing builder tests passed.
  The exact pinned and reference tool snapshots regenerated without drift.
- Codex CLI **0.169.0-alpha.11.1** completed DCR, browser approval and PKCE
  on the host fixture, listed tools in a second process, reconnected after
  an independent no-PIN restart, and refreshed after the saved access
  verifiers were invalidated. A host fixture does not exercise Cafe sockets.
- Gemini CLI **0.70.0** also paired on the host fixture, called a tool,
  restarted independently, and refreshed after simulated client-side expiry.

## Cemu

The isolated Cemu acceptance used the NSS-patched emulator
**e80e74c64334bdbf27151aa2c248a3beac12f42cfdf3322f570f433dfb99d19d**
with **--force-interpreter**. An earlier build reached the Night UI but
failed the TLS handshake when Cafe std rejected a send-timeout socket
option. The packaged build uses nonblocking Cafe sockets with absolute
deadlines and served TLS successfully.

The installed Gemini CLI **0.70.0** OAuth provider and MCP client made
direct HTTPS requests to the final RPX using the exported CA. The harness
drove consent and delivered the redirect to Gemini's real loopback callback.
Separate Gemini processes used the saved grant and called **versions**.
The **cemu-oauth-verified** run confirmed automatic service startup after a
no-PIN cold restart, followed by a PIN cold restart that waited for local
GamePad unlock. It used the original URL and CA in both modes. Aging
Gemini's local expiry timestamp triggered actual grant rotation. The test
requested a launcher-only access token and then recovered all four scopes
from the original consent with the rotated refresh token. No AI returned
HTTP 401 for MCP access and 403 for refresh. A separate local device
revocation returned HTTP 401 for access and 400 for refresh. The console
returned to Night and exited. **cemu-oauth-acceptance.json** records the
RPX and emulator hashes next to the package.

The repeatable Cemu runner is **wiiu/pregame/tests/cemu.ps1**. Supply the
NSS-patched Cemu executable, the packaged RPX, a new scratch output
directory, the guest's private address, and **-Production -NativeMcp**.
Use **-GeminiBundle** with the installed bundle module that exports the
OAuth provider and MCP client. **-RestartMcp** runs both restart cases.
The runner stores checksums and outcomes in **cemu-mcp-acceptance.json**.

The **cemu-game-final** run used the final RPX and the emulator interpreter.
Gemini requested **play** for a copied save, called **versions** during
preparation, and read **game_state** with a live player. It requested
**game_quit**, called Night again over the original HTTPS URL, and checked
that a fresh client could no longer discover the gameplay tool. Cemu logged
**hosted save barrier completed**, rewrote the copied **level.dat**, returned
to the Night Play screen, and exited cleanly. **cemu-game-acceptance.json**
records this run next to the package. The earlier **cemu-game-scoped** run
also passed the round trip against compatibility build **73f1d0b77a8**.

Installed Codex rejected the same-origin private IPv4 authorization
server during discovery. This matches an open upstream issue. A trusted
DNS hostname remains a separate Codex interoperability test.
[Codex issue 55163](https://github.com/openai/codex/issues/55163)

Gemini and Codex exercised the legacy transport in their respective runs.
A separate wire check against the packaged RPX in Cemu used the stateless
2026-07-28 transport. In independent HTTPS requests it discovered the server,
listed the private scoped catalog, called **versions**, and verified the
specified error when the tool name did not match the routing header. Rust
tests also cover per-client revocation and refresh replay. On the packaged
RPX, the TLS client rejected a certificate name mismatch. The server rejected
an unrecognized Host, a foreign Origin, anonymous tools and an oversized
request. It served metadata after an interrupted TLS handshake.
**cemu-https-validation.json** records the outcomes next to the package.

## Remaining checks

Physical Wii U testing remains. Verify NSS entropy, TLS exchanges over
the real LAN, storage recovery on FAT32 under power loss, TV and GamePad
consent, and the game save barrier with the exact package hash. The
legacy encrypted LAN diagnostic path is still present. Tool exclusions,
including console-side builds and dedicated servers, are in the parity
matrix. [Tool inventory](parity.md)
