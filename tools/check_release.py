#!/usr/bin/env python3
"""Validate release metadata, signatures, and the canonical documentation links."""
import argparse
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "wiiu/builder"))

import releases

CANONICAL_DOCS = ("README.md", "CONTRIBUTING.md", "docs/builder.md", "docs/inputs.md",
                  "docs/catalog.md", "docs/migration.md", "wiiu/builder/README.md")


def check_links(path):
    content = re.sub(r"^\s*```[\s\S]*?^\s*```[^\n]*", "", path.read_text(encoding="utf-8"), flags=re.M)
    for address in re.findall(r"(?<!!)\[[^\]]+\]\(([^\s)]+)\)", content):
        url = urlsplit(address)
        if not (url.scheme or url.netloc) and url.path and not (path.parent / unquote(url.path)).exists():
            raise ValueError(f"{path.relative_to(ROOT)} has a broken link to {address}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog", default=releases.CATALOG, type=Path)
    parser.add_argument("--signature", type=Path)
    parser.add_argument("--public-key", type=Path)
    parser.add_argument("--trust-state", type=Path)
    parser.add_argument("--client-asset", nargs=3, metavar=("VERSION", "KIND", "FILE"))
    parser.add_argument("--prepare-signing", action="store_true", help="validate unsigned bytes declared ready to sign")
    args = parser.parse_args()
    try:
        if any((args.signature, args.public_key, args.trust_state)):
            if not all((args.signature, args.public_key, args.trust_state)):
                raise ValueError("supply the signature, trusted public key and trust-state path together")
            catalog, _ = releases.verify_signature(args.catalog, args.signature, args.public_key, args.trust_state)
        else:
            catalog, _ = releases.read_catalog(args.catalog)
            if catalog["signing"] != "unsigned-development" and not args.prepare_signing:
                raise ValueError("signed catalog requires signature, key and trust state")
        releases.verify_release_files(catalog)
        if args.client_asset:
            if not all((args.signature, args.public_key, args.trust_state)):
                raise ValueError("provide the signature and pinned trust inputs to verify a client package")
            version, kind, path = args.client_asset
            print(releases.verify_client_asset(catalog, version, kind, Path(path)))
        for item in CANONICAL_DOCS:
            check_links(ROOT / item)
    except (ValueError, TypeError, KeyError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Release validation failed: {error}\n")
    print(f"Catalog schema {catalog['schema']}, sequence {catalog['sequence']}, {catalog['signing']}.")
    print("Pinned release artifacts and canonical documentation links verified.")


if __name__ == "__main__":
    main()
