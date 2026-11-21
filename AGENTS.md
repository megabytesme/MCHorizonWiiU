# Working in MCHorizonWiiU

- Read [CONTRIBUTING.md](CONTRIBUTING.md) for the branch workflow and validation commands. Use `main` for routine work and push each coherent, tested commit to `origin/main` immediately.
- Use one short-lived branch when work needs isolation. Push its commits as you make them, merge after acceptance, and remove the branch once its history and dependent work are accounted for.
- Keep CafeOS code in this repository and upstream changes in the exact-source patch series. Preserve the catalog's immutable Git pins and archive refs. Use the configured developer identity and preserve published history.
- Keep prepared resources, private SD state, account data, saves, signing material and development binaries out of Git. Use scratch Cemu and SD state for testing. Follow the [release procedure](docs/catalog.md) for signing and publication.
