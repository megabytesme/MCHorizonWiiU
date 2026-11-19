# Horizon Wii U port

This is the CafeOS port repository. It carries Horizon Night, the Wii U host
and native platform source, a local compatibility catalog and reviewed patch
artifacts. The builder reconstructs Horizon from an exact upstream Git object.

Start with the [builder](wiiu/builder/README.md), the [source and release
structure](docs/catalog.md), and the [launcher feature matrix](docs/launcher-features.md).
The [hosted game bridge contract](wiiu/pregame/INTEGRATION.md) covers the first
combined RPX.

The two build targets produce a standalone Horizon Night launcher and the
standalone native game. They have separate build and acceptance records. The
launcher shows Play as unavailable until the real game is integrated.
An explicit Cemu fixture build exercises the hosted mock session.
The catalog is an unsigned local release recipe. Use the accompanying pinned
inputs and build records to verify what went into the packages.
