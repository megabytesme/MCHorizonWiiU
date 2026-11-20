# Horizon Wii U port

This is the CafeOS port repository. It carries Horizon Night, the Wii U host
and native platform source, a local compatibility catalog and reviewed patch
artifacts. The builder reconstructs Horizon from an exact upstream Git object.

Start with the [builder](wiiu/builder/README.md), the [source and release
structure](docs/catalog.md), the [launcher feature matrix](docs/launcher-features.md), and the
[build and acceptance record](docs/validation.md).
The [hosted game bridge contract](wiiu/pregame/INTEGRATION.md) covers the first
combined RPX. The [combined build and Cemu record](docs/hosted-validation.md)
tracks its acceptance separately from the standalone builds. The
[Night local services record](docs/night-local-validation.md) covers the
current combined candidate and its Cemu checks.

The combined build starts in Horizon Night and runs the actual Cafe game in
one RPX. Start Game uses an installed Horizon 26.3 preparation. Standalone
Night, standalone game, and the mock test fixture remain diagnostic targets.
Each package has its own build and acceptance record. The local catalogs
record hashes and compatibility. Publisher signing is pending.
