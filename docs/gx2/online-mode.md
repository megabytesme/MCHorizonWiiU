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
| Attributes, profile and profile certificates | Same hosted client agent, with in-memory bearer credentials where required | The controlled fixture accepted a synthetic bearer POST. The online-mode replay confirms session joining. Individual authenticated attributes and profile-certificate responses were outside its receipt. |
| Session joining and multiplayer | `net.rs` receives the encryption request, computes the server hash and calls `auth::join_server`. The resulting TCP session uses protocol 777. | The exact RC 36 RPX joined and rejoined a secure online-mode Java server in Cemu, received chunks and accepted movement. Physical qualification remains open. |
| Address and SRV resolution | `horizon-protocol/src/resolver.rs`, native address resolution and the project's DNS SRV implementation | Public HTTPS exercised regular DNS in Cemu. RC 42 reads configured Cafe resolvers when NetConf returns them. The Cemu fixture returned zero. A physical SRV alias and the explicit host-and-port control remain to be tested. |
| Services signing keys | `horizon-server-files/src/profiles.rs`, independent of the hosted account agent | The RC 38 native probe fetched and parsed both public profile-certificate signing keys. |
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

A follow-up with a test clock past the fixture certificate's validity
confirmed certificate expiry is enforced. A server that withheld its
response was interrupted after 15.041 seconds between the two probe
markers. The follow-up RPX SHA-256 is
`36851c160a846b457ce9fe4f05dca5a1246315cb914cdf2b527a68b65d89b457`.

The two source-bound host tests passed for the body size and UTF-8 checks
and for rejecting a redirect as success.

The Cafe deadlines cover TCP connection attempts and individual socket
receives. A hard end-to-end request deadline, a send deadline and
interruption of in-progress service calls remain open. Keep the diagnostic
separate from a live Microsoft identity.

## Controlled online-mode login

The user signed in to a test account through Night in a disposable Cemu
profile. Night refreshed that same profile after opening the exact RC 36
RPX. A loopback vanilla Java 26.3 server with protocol 777 and both online
mode and secure profiles enabled verified the account through Mojang's
session service. The server's default allowlist then refused entry. The
authorized test profile was added to that fixture's allowlist and the
full world-entry test was retried.

The allowlisted run joined the world. TV and GamePad stills show the
received superflat terrain, an entity and the first-person arm. GamePad
input moved the player 20.503 blocks horizontally according to the
server's saved position. The normal Disconnect action reached Night,
the server reported a clean disconnect and the guest recovered
234,880,920 GX2 bulk bytes. The same Night process joined again,
rendered both outputs and disconnected cleanly a second time. The guest
then reached the Wii U Menu exit. The run used RC 36 RPX SHA-256
`9943740b5a437af03aa7a712750409c792eddecb730a99e35c34dc68549ba78b`.

## Authenticated RC 43 join and rejoin

The exact RC 43 RPX
`356f9cf09d578b701b61aeb8044947d1e9fc6479dc5a49119938599ef0cf638a`
reused the prior four-scope MCP grant and previously approved disposable
Microsoft profile in the established forced interpreter. Codex selected
the profile through Night, waited for `ready`, read the saved loopback
bookmark and called `wiiu_multiplayer_join`.

A disposable vanilla Java 26.3 server with `online-mode`,
`enforce-secure-profile` and its allowlist enabled recorded two logins and
two clean disconnects. Both rounds displayed received terrain in
separate TV 1920×1080 and GamePad 854×480 captures. During the first round
the client moved 28.49 blocks horizontally. The same Night process
rejoined after authenticated disconnect, recovered 234,880,920 GX2 bulk
bytes after each round, and reached `coreinit.exit(0)`.

The explicit saved host and port exercised the online protocol and
session service. Individual service response captures, a production SRV
alias and the physical join/rejoin timing remain in the supervised queue.
[RC 43 qualification](rc43-qualification.md)

## Cafe profile service requests

The offline RC 36 replay exposed a different consumer with the same
overall-timeout problem. The RC 38 fix in
`horizon-server-files/src/profiles.rs` uses the Cafe connect and receive
deadlines for both profile lookups and the services signing keys.
A native probe linked the patched server-files crate and fetched the
production discovery and key documents in Cemu. It parsed two
profile-certificate signing keys. The probe RPX SHA-256 is
`2eecf06f4d82fcced5bd582bdce79c7d5dfee2ad475574ca913baef926208b0b`.

## Online-mode acceptance

Use Night's normal sign-in or unlock an existing selected profile in the
test environment. Use a controlled protocol-777 server with online mode
enabled. Cover initial join, world and chunk reception, movement, both
outputs, clean disconnection to Night and a second join. Test a direct
hostname with an explicit port first. Add the SRV case when the Cafe
nameserver path is available. Record the
server version, result and environment without player identifiers, tokens
or raw request headers. Use a supervised SD transfer for the physical run.

## Cafe resolver and request lifecycle follow-up

WUT 1.9.2 declares `netconf_get_assigned_dns` in
`nsysnet/netconfig.h` for the Wi-Fi and Ethernet interfaces. A standalone
diagnostic resolves the export dynamically and reports per-interface
status and resolver count. It built with Cafe Toolchain 1.1.0. Check
initialization, byte order and the active interface on hardware before
passing the configured nameservers to the existing UDP/TCP SRV resolver.
The standalone probe resolves the entry points dynamically. NSS-patched
Cemu 2.6 returned zero assigned resolvers on each interface. RC 42
integrates the bounded adapter and keeps the explicit host-and-port path
when NetConf returns zero. [RC 42 qualification](rc42-qualification.md)
[Probe build and console procedure](../../wiiu/diagnostics/cafe-netconf/README.md)

The deployed Cafe HTTPS correction uses connect and receive timeouts.
A single deadline and cancellation need to include synchronous DNS,
TCP connect, the TLS handshake, writes and reads. ureq installs its
custom TLS connector after DNS and TCP connect, so that hook alone
cannot enforce the full operation budget. A scratch connected-socket
wrapper compiled for Cafe and passed host tests for a trickling
response, a blocked write and cancellation of a blocked read. The
remaining work is to carry one deadline and cancellation signal
through resolution and connection, register the socket before
authentication, and verify the whole path with native TLS.
