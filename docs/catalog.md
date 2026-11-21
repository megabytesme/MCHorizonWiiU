# Compatibility catalog and release process

The [versioned catalog](../catalog/releases-v2.json) has independent builder, client, Horizon patch and toolchain release records. A compatibility row explicitly permits one version tuple on the Cafe target and records the bridge ABI and exact upstream commit. Each source reference identifies an immutable Git commit, path and SHA-256. It can be fetched as `https://raw.githubusercontent.com/megabytesme/MCHorizonWiiU/<commit>/<path>`. Catalog validation checks source inventories and their members at the pinned commits, including active development releases. Check out the matching builder revision to recreate that version. The selected release's builder verifies its local source inventory before running. Revoked selections fail resolution.

Stable releases require a signed catalog and stable component records. Development permits explicitly selected local unsigned drafts. Nightly rows must be approved individually against exact upstream commits. A GitHub nightly moving to a new revision never changes a pinned row. Build, Cemu and hardware evidence are separate catalog fields. Publishing a new client with runtime changes requires evidence keyed to its RPX SHA-256.

## Prepare release artifacts

From a verified port-only checkout, run the host checks in [Contributing](../CONTRIBUTING.md). Prepare independently versioned source archives from the reviewed commit. For example:

```sh
python3 -B tools/prepare_release.py builder 0.6.0 --revision "$reviewed_commit" --output "$scratch/releases"
python3 -B tools/prepare_release.py client 0.7.0 --revision "$reviewed_commit" --output "$scratch/releases"
python3 -B tools/prepare_release.py patch 0.4.0 --revision "$reviewed_commit" --output "$scratch/releases"
```

Each archive includes its versioned notes and a separate SHA-256 receipt. Use distinct tags such as `builder-v0.6.0`, `client-v0.7.0`, `patch-v0.4.0` and `toolchain-v1.0.0` only when publication is authorized. Source releases can serve builders who generate private client installations. Optional prebuilt RPX/WUHB packages need complete linked dependency notices, redistribution review, an asset-free packaging audit and matching build and hardware evidence. Use [prepare_prebuilt.py](../tools/prepare_prebuilt.py) to stage an approved build by RPX hash and verify the WUHB by repacking without content files. Publish only the reviewed package files and redacted receipts. The signed catalog can reference the exact GitHub release asset ID, filename, size and SHA-256. Use `tools/check_release.py --client-asset VERSION rpx FILE` with signed trust inputs to verify a downloaded package.

## Final signing with the maintainer certificate

This repository's catalog is explicitly unsigned until a production publisher certificate is selected. The signer uses P-256 ECDSA over SHA-256 with a detached 64-byte IEEE P1363 signature. The verifier needs the publisher's independently authenticated DER SubjectPublicKeyInfo. The raw JSON bytes are signed, so keep LF endings and never reformat after signing.

1. On the signing PC obtain a maintainer-owned ECDSA P-256 signing certificate. Install it with a protected, preferably non-exportable private key in `Cert:\CurrentUser\My`. Verify the certificate thumbprint with the owner and keep the private key out of Git, CI and release artifacts.
2. Update the reviewed candidate's `signing` to `p256-sha256-p1363`. Advance `sequence` and set a UTC validity interval. Validate the draft with the builder catalog parser and confirm immutable artifact pins before signing.
3. Export only its public key, sign, and verify in a clean scratch directory. These commands use .NET 10.

```powershell
$signer = 'tools/catalog-signer'
python -B tools/check_release.py --prepare-signing
dotnet run --project $signer -- export-public "$scratch/catalog-public.der" $thumbprint
Get-FileHash "$scratch/catalog-public.der" -Algorithm SHA256
dotnet run --project $signer -- sign catalog/releases-v2.json "$scratch/releases-v2.sig" $thumbprint
python -B tools/check_release.py --signature "$scratch/releases-v2.sig" --public-key "$scratch/catalog-public.der" --trust-state "$scratch/trust.json"
```

4. Publish the verified signature with the exact signed bytes. Deliver the public key or its SHA-256 fingerprint through an independently authenticated channel. A key supplied by the same untrusted download does not establish publisher identity. Plan a reviewed key rotation and an explicit trust migration before retiring a key.

The desktop remembers the highest valid sequence and its bytes per trusted key in per-user local state. It rejects expired catalogs, older sequences and a different digest at the same sequence. Deleting state or changing the system clock can weaken that protection. A Wii U SD card is replaceable, so it cannot supply a tamper-proof counter. The console currently selects installed versions and does not fetch or install catalog updates.
