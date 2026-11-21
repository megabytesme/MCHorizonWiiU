#!/usr/bin/env python3
"""Create independently versioned source release candidates from reviewed Git bytes."""
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "wiiu/builder"))

import core
import releases
from source import SHA1


COMMON = ("LICENSE", "THIRD_PARTY_NOTICES.md")
DIRECTORIES = {
    "builder": ("wiiu/builder", "tools/catalog-signer", "tools/build_cafe_game_isolated.sh",
                "tools/check_release.py", "tools/prepare_release.py", "tools/prepare_prebuilt.py",
                "tools/wiiu_recompress_rpx.py", "catalog", "docs", "README.md", "CONTRIBUTING.md"),
    "client": tuple(core.CLIENT_PATHS) + ("wiiu/game-bridge", "wiiu/security", "wiiu/microsoft",
                                          "wiiu/mcp", "docs/mcp", "docs/gx2"),
    "patch": ("patches", "catalog", "wiiu/game-bridge", "wiiu/pregame/src/local_wire.rs",
              "wiiu/security", "wiiu/microsoft", "wiiu/mcp", "docs/mcp/pinned-tools.json"),
    "toolchain": ("patches/toolchain", "catalog/toolchain-cafe-v1.json", "catalog/dependencies-cafe-v1.json", "docs/inputs.md"),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kind", choices=DIRECTORIES)
    parser.add_argument("version")
    parser.add_argument("--revision", required=True, help="reviewed port-only commit")
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    try:
        core.pattern(args.revision, SHA1, "reviewed port commit")
        catalog, _ = releases.read_catalog()
        record = catalog["releases"][args.kind].get(args.version)
        if record is None or record["status"] != "active":
            raise ValueError("select an active version from the release catalog")
        releases.verify_release_files(catalog)
        for key, artifact in record["artifacts"].items():
            if "commit" not in artifact:
                continue
            revision_bytes = core.git(ROOT, "cat-file", "blob", f'{args.revision}:{artifact["path"]}').stdout
            if core.digest(revision_bytes) != artifact["sha256"]:
                raise ValueError(f'release revision does not contain {artifact["path"]}')
            if key == "sources":
                releases.verify_source_inventory(artifact, revision=args.revision)
        output = core.isolated(args.output)
        output.mkdir(parents=True, exist_ok=True)
        name = f"mchorizonwiiu-{args.kind}-v{args.version}"
        archive = output / f"{name}.zip"
        receipt = output / f"{name}.json"
        if archive.exists() or receipt.exists():
            raise ValueError("choose a new destination for this release candidate")
        references = [artifact["path"] for artifact in record["artifacts"].values() if "commit" in artifact]
        items = (*COMMON, *DIRECTORIES[args.kind], record["notes"], *references)
        tree = core.git(ROOT, "ls-tree", "-rz", args.revision, "--", *items).stdout
        if any(not entry.startswith((b"100644 ", b"100755 ")) for entry in tree.split(b"\0") if entry):
            raise ValueError("source archives require ordinary Git blobs")
        data = core.git(ROOT, "archive", "--format=zip", f"--prefix={name}/", args.revision, "--", *items).stdout
        with archive.open("xb") as stream:
            stream.write(data)
        core.write_record(receipt, {"format": "source", "kind": args.kind, "version": args.version,
                                    "revision": args.revision, "channel": record["channel"],
                                    "sha256": core.digest(data), "bytes": len(data)})
        print(f"{core.digest(data)}  {archive}")
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Release preparation failed: {error}\n")


if __name__ == "__main__":
    main()
