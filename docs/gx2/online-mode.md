# Outbound Minecraft Services and online-mode qualification

## RC 35 failure and RC 36 correction

Night starts its Microsoft device-code and refresh work in the
`horizon-wiiu-microsoft` worker. Its ureq 2.12.1 agent already uses separate
connect and read deadlines on Cafe. After Night passes an active identity
in memory, `crates/horizon-client-render/src/auth.rs` builds a different ureq
agent. Its overall request timeout makes ureq `connect_host` call
`TcpStream::set_write_timeout(Some(...))` after TCP connect and before
`connect_https` calls the TLS connector. CafeOS nsysnet does not support
`SO_SNDTIMEO`, so the initial discovery request fails before TLS.

RC 36 uses the existing Cafe TCP connect timeout and the standard library's
receive-timeout implementation for hosted client requests. For Minecraft
Services the Cafe agent requires HTTPS, disables redirects and accepts
only 2xx responses as success. It bounds JSON bodies at 1 MiB and retains
the pinned ureq and rustls stack, WebPKI roots, SNI, hostname validation and
certificate time checks. The skin downloader uses the same Cafe timeout
configuration and retains its vanilla texture URI policy. Desktop keeps
its existing overall timeout.

## Transport paths

| Operation | Production path | Evidence |
| --- | --- | --- |
| Microsoft device sign-in, refresh, Xbox, XSTS, entitlements, profile | Night worker, `wiiu/microsoft/src/protocol.rs`, fixed HTTPS endpoints and bounded ureq responses | Existing identity handoff reproduced the RC 35 field failure. RC 36 interactive flow pending. |
| Discovery and public session endpoint resolution | Hosted `horizon-client-render/src/auth.rs` and `client_http.rs` | Genuine public discovery recovered in native Cemu. Hardware retest pending. |
| Attributes, profile and profile certificates | Same hosted client agent, with in-memory bearer credentials where required | The controlled fixture accepted a synthetic bearer POST. Authenticated production calls remain pending. |
| Session joining and multiplayer | `net.rs` receives the encryption request, computes the server hash and calls `auth::join_server`. The resulting TCP session uses protocol 777. | Public discovery resolved the genuine session endpoint. A real authenticated join, encryption, compression, configuration and playable world remain pending. |
| Address and SRV resolution | `horizon-protocol/src/resolver.rs`, native address resolution and the project's DNS SRV implementation | Public HTTPS exercised regular DNS in Cemu. Cafe uses the empty system-nameserver fallback for SRV, so use a direct hostname and explicit port until a Cafe resolver is available. |
| Incoming Agent Control | `wiiu/mcp/src/tls.rs`, a listener with its own rustls server configuration and pairing | Existing physical authenticated MCP evidence. |

## Verification

The public discovery probe compiled the production client HTTP helper with
Cafe Toolchain 1.1.0 and the candidate's locked ureq stack. On NSS-patched
Cemu 2.6 in forced interpreter mode, the old overall-timeout control
returned a transport I/O error. The corrected arm returned a successful
HTTPS status, parsed the production JSON, and resolved the expected
session-join URI. It rejected explicit plain HTTP. The probe RPX SHA-256 is
`40f22dd9c771d889c454aad404bee55a632d3f91079c62b9e46f446cd31270b5`.
[Production discovery document](https://discovery.minecraftservices.com/minecraft/client)

A separate native probe used a loopback HTTPS server and a generated
fixture CA. It verified SNI, JSON GET, a synthetic bearer and JSON POST,
HTTP 204, an unfollowed 302 and a body above the 1 MiB limit. It also
rejected a certificate for the wrong hostname and a chain rooted in an
untrusted CA. The fixture never received a redirected request. The probe
RPX SHA-256 is
`43ebbdc01840c705418fd889482e3fc60e66f5cc393e3f41af1a5e11f3f88bc0`.

The two source-bound host tests passed for the body size and UTF-8 checks
and for rejecting a redirect as success.

The Cafe deadlines cover TCP connection attempts and individual socket
receives. A hard end-to-end request deadline, a send deadline and
interruption of in-progress service calls remain open. Keep the diagnostic
separate from a live Microsoft identity.

## Online-mode acceptance

Use Night's normal sign-in or unlock an existing selected profile in the
test environment. Use a controlled protocol-777 server with online mode
enabled. Cover initial join, world and chunk reception, movement, both
outputs, clean disconnection to Night and a second join. Test a direct
hostname with an explicit port first. Add the SRV case when the Cafe
nameserver path is available. Record the
server version, result and environment without player identifiers, tokens
or raw request headers. Use a supervised SD transfer for the physical run.
