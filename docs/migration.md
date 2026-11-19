# Migration checkpoints

The full-fork baseline and launcher foundation remain in their own worktrees.
The port-only branch has independent source history. A future remote can
receive this branch alone. No remote publication was part of this migration.

| Preserved checkpoint | Commit |
| --- | --- |
| Shared launcher core for the visual comparison | `0e99cacd6c30d5d56cf1ed06aef5dfd46d0df806` |
| Original Horizon Night candidate | `27f256f3d4bda7664254fdec56bc630ea6ebd7ee` |
| Wii U Native design reference | `33cd634c9ff755e0c715d25da5631f4cf04b9e8e` |
| Hosted-session launcher baseline | `f4ac01d8915d09cfad7de6d0117246b0d4da5449` |
| Canonical native source used by the migration | `1a0a2948c33d11744eb403247cb566b0aa3ee858` |
| Separate game-agent diagnostic | `7c8ad024b49c5d813862c826931b91ff20fc6d83` |

The hosted-session baseline passed 10 host tests, rustfmt, Clippy, the Cafe
build, package hash verification, and the isolated Night Cemu fixture. The
fixture exercised mock preparation, save completion followed by resource
release, cancelled startup, failed startup, repeat launch and guest exit.
The committed manifest-built RPX matched the tested binary.

| Hosted baseline artifact | SHA-256 |
| --- | --- |
| `HorizonLauncher-Night.rpx` | `4b5832c8be9853ed3acdd2d4dc7d6f6152b1b20638db279b6b00a4ce5ca34136` |
| `HorizonLauncher-Night.wuhb` | `2aac341af29f560877b341d2085357054b16a371d57b02fe50d26fee285231b2` |

Review [the implemented port layout](catalog.md), [build inputs](inputs.md),
and [the hosted game contract](../wiiu/pregame/INTEGRATION.md) before
maintaining the migrated branch. To get a clean Linux build checkout from a
local object store, clone only `codex/wiiu-port-structure` with
`git clone --single-branch --branch codex/wiiu-port-structure <local-repo> <scratch-path>`.
The upstream source object remains a separate builder input.
