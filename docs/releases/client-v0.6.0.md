# Wii U client 0.6.0

The Night launcher supports offline local profiles, compatible installations, saved-world selection and handoff to the native Cafe game. GamePad controls and TV rendering use the native host.

Two fresh builds on Ben-Desktop applied patch 0.3.1 to the exact public Horizon source and compiled with Cargo offline in WSL 2. Both produced byte-identical packages. The builder bound the project and output to fixed compiler-visible paths. The [machine-readable build record](../../catalog/evidence/client-v0.6.0-patch-v0.3.1.json) lists the source and recipe hashes, package sizes and tool hashes.

| Artifact | SHA-256 |
| --- | --- |
| `HorizonWiiU.rpx` | `58b3ce0dbea4637bc208cbd826f0165ac754a7d9b709a6e11087d215da59104b` |
| `HorizonWiiU.wuhb` | `7b45a9582516668e98fdce892af29608b494d80378be9b86e037e82cd1a5977b` |

The exact public-candidate RPX reached the Horizon Night Home screen in an isolated Cemu 2.6 portable profile. The [original Night record](../night-local-validation.md) covers the broader local-profile and save-cycle tests for earlier package hashes. Physical acceptance of this candidate remains pending, including saved-world reopening.
