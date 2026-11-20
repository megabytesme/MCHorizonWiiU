# Builder 0.5.0

The Windows desktop UI calls the release-aware CLI in WSL. It checks the Cafe toolchain, prepares verified dependency and resource inputs, builds the combined RPX and WUHB, and passes cancellation through to native tools. The output receipt records package hashes and provenance. See the [builder guide](../builder.md).
