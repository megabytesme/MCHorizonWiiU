# Wii U client 0.7.0

The combined development build includes Horizon Night, Microsoft accounts,
the shared vault, native HTTPS MCP, original gameplay and the GX2 updates.

| Package | Bytes | SHA-256 |
| --- | ---: | --- |
| `HorizonWiiU.rpx` | 36,339,742 | `003d27df3db81f099adbba99783ef194ae0af12a9e21a28075389870a11fdc47` |
| `HorizonWiiU.wuhb` | 36,340,556 | `b86134dd2ee2daaf963e184b15e869613b9c775cedb9896eed3d24b72c759325` |

## Build and verification

Ben-Desktop built under Ubuntu WSL 2 using the catalog workflow and the
fixed compiler-visible path. The upstream source, each hosted patch stage,
private overlays and toolchain all passed their pinned checks. The resulting
hosted source tree is `ec14a2de4bb314b8ad72cfac859767be93c62aff`.
The catalog workflow verified the prepared-resource inventory and produced
both packages from reviewed build source
`8df2cf1f329e841493cc8613be9fa8d90ba6f695`. Two independent clean
fixed-path builds produced byte-identical RPX and WUHB files. Repacking
the RPX with only the standard metadata reproduced the WUHB.
[Hosted recipe](../../catalog/hosted-v3.json)

```sh
python3 -B wiiu/builder/workflow.py --allow-unsigned --offline \
  --cache "$cache" --upstream "$upstream" prepare --inputs "$inputs"
python3 -B wiiu/builder/workflow.py --allow-unsigned --offline \
  --cache "$cache" --upstream "$upstream" build --fixed-path \
  --inputs "$inputs" --output "$scratch/combined"
python3 -B wiiu/builder/build.py verify-output "$scratch/combined/build-record.json"
```

Host checks covered 43 Rust tests across vault, Microsoft, MCP and Night,
plus six GX2 projection and atlas cases, three profiler decoder tests and 15 builder
tests. Night formatting and Clippy passed in WSL. The public-tree check,
redacted Gitleaks scan, pinned tool inventory regeneration, catalog checks
and ephemeral signer self-test passed. An independent diagnostic assembly
reproduced the existing world-reopen diagnostic tree. The standalone Night
package built with the verified Cafe Rust runtime and booted in stock Cemu. The standalone game also packaged successfully.

Rust dependency and target build paths were remapped where possible. The
public-prebuilt gate still found two pinned SDK paths in `.rodata` and four
paths from the precompiled Rust dependency graph in `.data`. Public binary
redistribution remains blocked. The catalog remains unsigned for private
development pending legitimate publisher configuration and release review.
The production RPX passed the check for absence of Microsoft fixture
literals.

## Validation results

| Environment | Evidence covered |
| --- | --- |
| Host fixtures | Credential domain isolation, protected restart and wrong-PIN refusal, automatic unlock, session-only storage, migration recovery, Microsoft token-refresh and entitlement behavior, world ownership and Night persistence. Also covers TLS IP and DNS certificate validation, PKCE, consent, refresh replay, scopes and session-generation cancellation. |
| Stock Cemu 2.6 interpreter | The exact RPX stopped at the Cafe system RNG startup check. |
| NSS-patched Cemu 2.6 interpreter | The exact RPX booted with the real Night backend, displayed the Microsoft service and persisted a scratch offline profile and controller setting across a cold restart. Gemini completed direct OAuth and called the real Night versions tool with the validated CA. The modern stateless protocol checks passed as well. Gemini completed two saved-world sessions through the same HTTPS endpoint. Both saves returned to Night and withdrew game tools. Independent cold restarts verified automatic unlock, local PIN unlock, refresh rotation, per-client revocation and No AI. |
| Physical Wii U | Pending. Instance A's saved-world reopening investigation remains separate. |

| Combined acceptance | Result |
| --- | --- |
| 1â€“3. Night, offline persistence and account initialization | Passed on NSS-patched Cemu. The real Night backend displayed the Microsoft account service. |
| 4â€“5. Protected and automatic credential restore | Microsoft account storage passed host fixtures for all three modes. The combined Cemu MCP vault passed protected cold restart and automatic restore. |
| 6. Microsoft identity handoff | The verified identity is wired into the hosted game and save owner checks. The protocol fixture passed. A real signed-in Microsoft session and online multiplayer handoff remain untested on this RPX. |
| 7â€“9. Persistent HTTPS identity, direct Gemini OAuth and real launcher tools | Passed with the guest's exported CA and GamePad approval. Gemini used the legacy protocol and the direct wire fixture passed modern stateless requests. |
| 10â€“11. Reconnect, scoped refresh, revocation and No AI | Passed in the Cemu restart suite. Local PIN unlock preserved the trusted grant. |
| 12â€“16. Real game launch, gameplay tools, Save & Quit, withdrawal and a fresh session | Both copied-world launches passed within one application process. The copy's level data changed, both save barriers completed, the same HTTPS origin served launcher tools, and the game catalog withdrew at each shutdown. Host tests also rejected stale generations. |
| 17â€“18. GX2 and profiler captures | Terrain and HUD appeared on TV and GamePad. The controlled lightmap and the profile decoder passed. Full visual parity and physical performance remain open. |
| 19â€“20. Source and output verification | Catalog-selected offline builds verified the pinned patch trees, dependency overlays, toolchain, Cargo lock, private resource inventory and output hashes. Clean builds matched. Public tree, secret scan and catalog checks passed. The separate prebuilt publication gate remains blocked. |

## Repeat the Cemu acceptance

Use the cold-start and gameplay suites in
`wiiu/pregame/tests/cemu.ps1`. Both take `-Production -NativeMcp`,
the final RPX, the NSS-patched Cemu executable, a fresh output directory
and the installed Gemini CLI bundle via `-GeminiBundle`. Use
`-RestartMcp` for the launcher-only restart suite. For the game suite,
supply a private prepared-resource copy, an existing disposable save and
its options with `-PreparedResources -IsolatedSaves -IsolatedOptions`.
Set `MCHORIZON_CEMU_ROUND_TRIPS=2` and
`MCHORIZON_CEMU_GAME_EVIDENCE` to the same fresh output directory. Add
`-CaptureProfile` to enable the bounded GX2 recording.

## GX2 captures on the mapped-atlas build

Both saved-world sessions at render distance 12 produced visible terrain and HUD
on both outputs while sections were still streaming. The first session capture
retained 274 presented Playing iterations, with a peak of 112 section draws
and 85,068 indices per iteration. The same capture counted 266 worker
completions. The HTTPS endpoint served launcher requests during world entry,
gameplay and save return.

The stock Cemu lightmap fixture compiled from the final staged terrain
pipeline passed eight checks across both outputs. Both displays sampled
RGB (127, 42, 32) for the interpolated reference value (128, 43, 32)
with tolerance 2. Diagnostic RPX SHA-256
`aac71aae855d283f9dcd14fc5a76fc495b03e7eac340160c17e3742973e66680`.

## Earlier integrated replay and GX2 baseline

The earlier combined RPX `8ec030542e756d3e0f81f320025d4d483c7f195a3e30187270625a6b47ab47bb`
completed one direct Gemini saved-world session and Save & Quit. Its next
launch reached the renderer and failed at terrain initialization with code
`-2`. Night recovered on the same endpoint. The mapped-atlas patch removes
the additional 64 MiB CPU texture buffer and reports the exact native stage
on future allocation failure.

The earlier RPX presented the in-game HUD on TV and GamePad during
saved-world entry at render distance 12. The captured early frames showed
sky and HUD while terrain was still streaming. The marker-enabled profile
recorded 253 presented iterations in the Playing phase with a peak of 40
section draws and 33,048 indices, plus 205 worker completions across that
phase. Night, loading, entry and closing profiles decoded successfully.
The HTTPS endpoint continued handling launcher requests during the run.

The earlier independent lightmap fixture used the staged
terrain pipeline. On stock Cemu, it passed eight pixel checks across TV
and GamePad. Both displays sampled RGB (127, 42, 32) for the interpolated
reference value (128, 43, 32) within tolerance 2. The fixture RPX SHA-256
was `fe353ead59bb0a5f909bf52b80775ee0ee98ca2dd4b082d301c1966238e38517`.
These are interpreter results for world entry and a controlled visual
diagnostic. Steady gameplay timing and physical GPU performance remain open.
[GX2 coverage matrix](../gx2/coverage.md)

The tested Codex CLI rejected OAuth discovery for the private IPv4 origin.
A LAN hostname resolved locally and served OAuth metadata over validated
TLS. Direct Codex authorization did not complete in that test window.
[Codex issue 55163](https://github.com/openai/codex/issues/55163)

Live Microsoft account authorization, online multiplayer identity, physical
console HTTPS and full GX2 visual parity require their own validation.
The two new Microsoft registrations still need Minecraft API approval.

[Source inventory, integration decisions and review steps](../unified-integration.md)
