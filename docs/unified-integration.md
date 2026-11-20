# Unified MCHorizonWiiU development candidate

The candidate is based on the sanitized port-only review history. The data
below distinguishes the original feature runs from tests on the combined RPX.
Physical saved-world reopening remains Instance A's independent investigation.

## Verified source inputs

| Input | Branch | Commit |
| --- | --- | --- |
| Canonical base | `codex/wiiu-canonical-release` | `60ab798f3f53dc859420480c9bfb38bada46a8a8` |
| Common feature baseline | Original port-only history | `cf2ac8cbbad809a878e59aa4b20288a551294283` |
| S | `codex/wiiu-vault-accounts` | `9f12731741662615c6f1e58d09a414d2293aada5` |
| M | `codex/wiiu-native-http-mcp` | `9a6be7431678bc87ce7c3788c0bdd11656c94cfc` |
| M packaged source | HTTPS/OAuth acceptance package | `aacb5f9a15464b4d942bcc5a7e92fe3605b4808d` |
| G | `codex/wiiu-gx2-performance` | `0ed27db69eac2592a183b4e466565481ed2882e9` |
| G packaged source | Latest GX2 acceptance package | `beb945ea20a` |
| Pinned Horizon | Original upstream | `69fab597a6f0e8a6ce524d38d1b73e599e343034` |

All four port inputs had clean worktrees. The historical feature sources and
build evidence remain in their original locations. The original full-game
checkout already had local work in progress and was left untouched.

| Area | S | M | G | Integration decision |
| --- | --- | --- | --- | --- |
| Vault | Argon2id, AEAD, migration, retirement, domain erase | Imports early S vault checkpoints | | Keep S's final crate, including retirement flush. MCP owns one vault domain and Microsoft owns per-profile domains. |
| Night | Profiles, unlock state, protected keyboard, private input buffer cleanup | HTTPS settings, GamePad consent, local tools, restart/revocation UI | Night profile hooks in Cafe C host | Keep S's account state in the live app for the entire launcher/game cycle. Add a distinct `OPEN_AGENT` command and dispatch local input to one menu owner. |
| Game bridge | Verified game identity and save ownership | Launch/return, command dispatch and session generations | Camera, lightmap, save phase and pacing | Keep the online handoff. Queue scoped launch intent and revalidate it at the hosted entry. Withdraw game tools before starting the save barrier and record the Closing phase at the same point. |
| Cafe host | Private system keyboard | Hosted MCP network polling | Frame profiling and WHB pacing | Preserve keyboard masking and buffer clearing, local approval, and both TV/DRC outputs. Profiling stays marker-enabled. |
| Horizon deltas | Auth/session join, LAN identity, Cafe socket deadlines | Hosted control hook and No AI coupling | Camera/effects, shared lightmap, instrumentation and native shader dependencies | Apply canonical hosted base, S, M, then G. Regenerate binary-safe patches and verify the staged tree after each step. |
| Native dependencies | `ureq`, `rustls`, shared vault and lock | `rcgen`, `rustls`, shared vault and lock | Profile dependency and modified hosted lock | Use the verified Cafe dependencies and SDK, with one hosted Cargo lock. Cargo resolved the Microsoft crate into the MCP graph offline. |
| Builder/catalog | Local account overlay and alternate lock | Local MCP overlay and alternate lock | Branch-only recipe and patch list | Keep canonical standalone and diagnostic pins. Add hosted recipe 3 and the development tuple builder 0.6.0, client 0.7.0 and patch 0.4.0 with toolchain 1.0.0. Preserve older catalog records and signed-catalog verification. |

The Microsoft ID is the tested public UWP registration. The two pending
registrations require their own Minecraft API approval. Entitlement checks,
explicit no-PIN warnings, session-only storage and save identity checks
are retained. Account tokens are handed to the game in memory. No
launcher tool lists, unlocks, or reads Microsoft credentials. The native
MCP audit still maps 44 of 192 pinned tools. [MCP matrix](mcp/parity.md)

The earlier standalone feature builds are documented in
[MCP validation](mcp/validation.md) and [GX2 verification](gx2/verification.md).
The GX2 timing comparison covered loading. Hardware and full renderer parity
remain open.

## GitHub review and release

1. Have the maintainer approve pushing the local candidate. Run
   `git push -u origin codex/wiiu-unified-candidate` from this worktree, then
   open a draft review against the sanitized branch with
   `gh pr create --repo megabytesme/MCHorizonWiiU --draft --base codex/wiiu-canonical-release --head codex/wiiu-unified-candidate --body-file "$pr_body"`.
   Use the authenticated account's agent attribution and include exact RPX
   hashes and the validation matrix. Review this branch against
   `60ab798f3f53dc859420480c9bfb38bada46a8a8`. [GitHub PR command](https://cli.github.com/manual/gh_pr_create)
2. Require both host CI platforms, the public-tree and secret scan,
   source and dependency reconstruction, and review of the credential,
   game teardown and rendering deltas. When approved, use a merge commit
   to retain the source commits referenced by the catalog.
   `gh pr merge PR_NUMBER --repo megabytesme/MCHorizonWiiU --merge`.
3. After a separate production approval, select the maintainer-owned P-256
   certificate, advance the catalog sequence and validity interval, sign the
   exact bytes, and verify from a clean scratch trust store. Keep binaries
   private until the path audit, linked-dependency redistribution review and
   required hardware checks pass. [Signing and publication](catalog.md)
4. The legacy `main` and port-only histories have separate roots. After
   separate authorization for cutover, have a repository administrator set
   the reviewed port-only branch as default with
   `gh repo edit megabytesme/MCHorizonWiiU --default-branch codex/wiiu-canonical-release`.
   Verify `git ls-remote --symref origin HEAD refs/heads/main refs/heads/archive/legacy-full-horizon`,
   update rulesets and release automation, and preserve both legacy refs.
   Rollback is switching the default setting back to `main`.
   [Existing migration record](migration.md) · [GitHub branch settings](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-branches-in-your-repository/changing-the-default-branch)


## Exact feature file inventory

Paths are from each feature tip against the common baseline. They include feature-only overlays, catalogs and locks superseded by the canonical recipe.

<details>
<summary>S + M (10 paths)</summary>

- `wiiu/builder/build.py`
- `wiiu/pregame/Cargo.toml`
- `wiiu/pregame/src/cafe.rs`
- `wiiu/security/Cargo.lock`
- `wiiu/security/Cargo.toml`
- `wiiu/security/README.md`
- `wiiu/security/src/format.rs`
- `wiiu/security/src/lib.rs`
- `wiiu/security/src/vault.rs`
- `wiiu/security/src/vault/tests.rs`

</details>

<details>
<summary>S + M + G (1 paths)</summary>

- `wiiu/builder/native.py`

</details>

<details>
<summary>S only (19 paths)</summary>

- `wiiu/builder/README.md`
- `wiiu/builder/core.py`
- `wiiu/game-bridge/accounts.patch`
- `wiiu/game-bridge/night_hosted.rs`
- `wiiu/game-bridge/night_menu.rs`
- `wiiu/game-bridge/night_online.rs`
- `wiiu/game-bridge/offline/accounts.Cargo.lock`
- `wiiu/microsoft/Cargo.lock`
- `wiiu/microsoft/Cargo.toml`
- `wiiu/microsoft/src/fixture.rs`
- `wiiu/microsoft/src/lib.rs`
- `wiiu/microsoft/src/profiles.rs`
- `wiiu/microsoft/src/protocol.rs`
- `wiiu/microsoft/src/tests.rs`
- `wiiu/microsoft/src/worker.rs`
- `wiiu/pregame/Cargo.lock`
- `wiiu/pregame/README.md`
- `wiiu/pregame/src/local_wire.rs`
- `wiiu/pregame/tests/local.rs`

</details>

<details>
<summary>M only (34 paths)</summary>

- `docs/launcher-features.md`
- `docs/mcp/README.md`
- `docs/mcp/parity.json`
- `docs/mcp/parity.md`
- `docs/mcp/pinned-tools.json`
- `docs/mcp/reference-tools.json`
- `docs/mcp/validation.md`
- `tools/mcp_codex_cemu_test.py`
- `tools/mcp_codex_host_test.py`
- `tools/mcp_gemini_cemu_test.py`
- `tools/mcp_gemini_host_test.mjs`
- `tools/mcp_gemini_host_test.py`
- `tools/mcp_inventory.py`
- `tools/mcp_parity.py`
- `wiiu/builder/mcp_native.py`
- `wiiu/mcp/Cargo.lock`
- `wiiu/mcp/Cargo.toml`
- `wiiu/mcp/cafe/hosted.patch`
- `wiiu/mcp/cafe/night.Cargo.lock`
- `wiiu/mcp/cafe/night_mcp.rs`
- `wiiu/mcp/cafe/night_mcp_menu.rs`
- `wiiu/mcp/cafe/night_tools.rs`
- `wiiu/mcp/examples/fixture.rs`
- `wiiu/mcp/src/http.rs`
- `wiiu/mcp/src/lib.rs`
- `wiiu/mcp/src/oauth.rs`
- `wiiu/mcp/src/protocol.rs`
- `wiiu/mcp/src/router.rs`
- `wiiu/mcp/src/service.rs`
- `wiiu/mcp/src/session.rs`
- `wiiu/mcp/src/tls.rs`
- `wiiu/pregame/CMakeLists.txt`
- `wiiu/pregame/src/launcher.rs`
- `wiiu/pregame/tests/cemu.ps1`

</details>

<details>
<summary>G only (18 paths)</summary>

- `README.md`
- `catalog/hosted-v2.json`
- `docs/gx2/api.md`
- `docs/gx2/benchmarks.md`
- `docs/gx2/coverage.md`
- `docs/gx2/verification.md`
- `patches/69fab597a6f0e8a6ce524d38d1b73e599e343034/hosted/0003-gx2-frame-capture.patch`
- `patches/69fab597a6f0e8a6ce524d38d1b73e599e343034/hosted/0004-gx2-camera-pose.patch`
- `patches/69fab597a6f0e8a6ce524d38d1b73e599e343034/hosted/0005-gx2-stock-lightmap.patch`
- `patches/69fab597a6f0e8a6ce524d38d1b73e599e343034/hosted/0006-gx2-whb-pacing.patch`
- `patches/69fab597a6f0e8a6ce524d38d1b73e599e343034/hosted/0007-gx2-shader-relink.patch`
- `tools/check_gx2_lightmap.sh`
- `tools/gx2_cemu.py`
- `tools/gx2_profile.py`
- `tools/test_gx2_profile.py`
- `wiiu/game-bridge/offline/night.Cargo.lock`
- `wiiu/gx2_terrain/tests/lightmap.c`
- `wiiu/host/cafe_host.c`

</details>
