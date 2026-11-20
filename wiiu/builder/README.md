# Wii U builder implementation

Start with the [desktop and CLI guide](../../docs/builder.md), [input inventory](../../docs/inputs.md) and [release catalog](../../docs/catalog.md).

`workflow.py` is the release-aware CLI. It verifies the catalog, reuses exact upstream assemblies and prepares private resources. `desktop.py` uses the same workflow in a WSL job. `build.py` provides lower-level diagnostics and package verification.

```sh
python3 -B wiiu/builder/build.py assemble --source "$upstream_git" \
  --revision 69fab597a6f0e8a6ce524d38d1b73e599e343034 --output "$scratch/assembled"
python3 -B wiiu/builder/build.py verify-output "$scratch/combined/build-record.json"
```

The low-level assembly supports `--diagnostic` for the historical world-reopen trace. The low-level `build` command builds standalone Night. Add `--fixture` for the mock hosted/session fixture. The game-only and combined packages use `build-game` and `build-hosted`. The combined release workflow always uses the catalogued hosted client and patch selected in the desktop or CLI.

Use a clean port-only checkout with an isolated build directory. Thin Cemu checks and expected launch behavior are documented in the [Night validation record](../../docs/night-local-validation.md) and [launcher tests](../pregame/README.md).
