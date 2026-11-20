# Builder 0.5.0

The Windows desktop UI calls the release-aware CLI in WSL. It checks the Cafe toolchain, prepares verified dependency and resource inputs, builds the combined RPX and WUHB, and passes cancellation through to native tools. The output receipt records package hashes and provenance. See the [builder guide](../builder.md).

The Windows UI toolchain action matched the pinned Cafe inventory and its Cancel action stopped a WSL setup job with status 130. Host tests cover interrupted setup, low disk space, invalid cache bytes, incompatible version tuples, revocations and catalog rollback.
