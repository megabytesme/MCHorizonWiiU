# RC 47 source qualification

RC 47 uses the same upstream source, resource pins, builder and Cafe
Toolchain 1.1.0 as RC 46. Its source recipe sets the client to
`0.8.0-rc.47` and the patch series to `0.5.0-rc.47`.
[Exact Nightly recipe](../../catalog/hosted-nightly-2026-11-20.json)

| Patch | Input tree | Output tree |
| --- | --- | --- |
| 0169 Shorten Cafe atomic save staging | `037bcf9dc59e000d0de76509f3f8304f63a9c5f6` | `dea7b5ec95b9c552d45ae1b739d3a30501135fb3` |
| 0170 Recheck MCP authorization after a wait | `dea7b5ec95b9c552d45ae1b739d3a30501135fb3` | `499ba0705dce6703c521417ba92e2542380e074a` |

The builder reconstructed the RC 47 tree from the pinned upstream checkout.
All 26 builder host tests passed.
The MCP audit matches RC 46 across all 63 stock registrations and their
scopes. The upstream inventory contains 194 tools.
[Exact tool comparison](../mcp/nightly-parity.json)

The RC 45 Cemu log shows save errors when temporary save directories
reached 248–257 characters. A Windows host probe reproduced directory
creation failure at 248 characters and created a 255-character payload
under the longest replacement staging layout needed by that fixture.
The Cafe persistence module passed seven host tests, covering bounded
staging, recovery, rollback and overlapping writes.

The stock Night host test resumes a copied disposable grant through the
real HTTP and OAuth router. The RC 46 regression allowed a queued
bookmark addition and a delayed game-state reply after revocation.
In RC 47 the queued write is denied, the reply is withheld, and the
revoked client sees no tools. The test also checks that the saved grant
and refresh credential remain revoked after reopening the database,
and that pairing a new read-only client does not grant write or game
permissions.

```sh
python3 -B wiiu/builder/build.py assemble-hosted-nightly \
  --source "$nightly_checkout" --inputs "$cafe_inputs" --output "$candidate_output"
python3 -B tools/mcp_parity.py --check --nightly-source "$candidate_output/source"
python3 -B tools/mcp_night_revocation_test.py --source "$candidate_output/source"
rustc --edition 2024 --test \
  "$candidate_output/source/crates/horizon-world/src/cafe_persistence.rs" \
  -o "$candidate_output/cafe-persistence-tests"
"$candidate_output/cafe-persistence-tests"
```

The pinned Cafe Toolchain 1.1.0 build used 32 Cargo jobs on Ben-Desktop.
Cargo release took 17 min 32 s. The command was
`tools/build_cafe_game_isolated.sh --hosted-nightly` from port commit
`7f0bd61941b6ba39bf2f390c0ae55ac4fd7bb172` with output in
`/var/tmp/mchorizon-rc47-assembly.U2qLaBHa/out`. The build verified both hashes and
estimated 12,657,984 bytes of remaining Aroma code heap.

| Package | SHA-256 |
| --- | --- |
| RPX | `be1ea346a9fd6fac95f9c0248fe72e0cfc08015fcd183435f254fe2f1f4bc6b9` |
| WUHB | `0ad9f8282a31f2b11eef68ce1708210fd0615e9f5c6aae9121cddd8d8d4b5159` |

Exact-package Cemu and physical qualification remain open.
