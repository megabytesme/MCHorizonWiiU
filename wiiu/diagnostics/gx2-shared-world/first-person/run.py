#!/usr/bin/env python3
"""Test the pinned Cafe first-person crop geometry with an assembled source tree."""

import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile


# Exact stock RC46 source tree 037bcf9dc59e000d0de76509f3f8304f63a9c5f6.
# All these blobs also match scene tree 34092c4b1bbe5c53c1533e7384d965706489c18a.
INPUTS = {
    "camera": (
        "crates/horizon-client-render/src/camera.rs",
        "4ee7f45c43eb6852e784b25090cfe60002acda2d980a4806ac8a4cf51b2277c6",
    ),
    "gfx": (
        "crates/horizon-client-render/src/gfx.rs",
        "66689214ed959d00209e3e4f82b4775c6e1d6b6932c058cedd18729a805e357d",
    ),
    "trig": (
        "crates/horizon-core/src/geometry/trig.rs",
        "3a67dced40d39b3bf754094f464e1ffb1bd9818d9f74f0247f7444cd25b950c2",
    ),
    "jmath": (
        "crates/horizon-core/src/jmath.rs",
        "fa93fdf451c7e86ecd98192a41689ad583dedcbcbef37e9ea2d0fae6f7ab096e",
    ),
    "terrain": (
        "wiiu/gx2_terrain/src/lib.rs",
        "4813d0c48e2e4083f4634410235afd4715c4ab5bbb799c27f30f294ec86474cc",
    ),
    "hands": (
        "wiiu/loading_host/hand.rs",
        "2d632534758222590e6a26411439744ccb2b9dfa64d420c31a03495a1c89658c",
    ),
    "outline": (
        "wiiu/loading_host/outline_vertices.rs",
        "64bb13618005e82fb3d241c035f6310e64f9cc8aeb781f3a0b418187dd8e5223",
    ),
}
MANIFEST = """[package]
name = "cafe-first-person-crop-fixture"
version = "0.1.0"
edition = "2024"

[dependencies]
glam = "=0.30.10"
libm = "=0.2.16"
"""
LOCK = """version = 4

[[package]]
name = "cafe-first-person-crop-fixture"
version = "0.1.0"
dependencies = [
 "glam",
 "libm",
]

[[package]]
name = "glam"
version = "0.30.10"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "19fc433e8437a212d1b6f1e68c7824af3aed907da60afa994e7f542d18d12aa9"

[[package]]
name = "libm"
version = "0.2.16"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "b6d2cec3eae94f9f509c767b45932f1ada8350c4bdb85af2fcab4a3c14807981"
"""


def read_inputs(root):
    sources = {}
    for name, (relpath, expected) in INPUTS.items():
        path = root / relpath
        # Accept either Git checkout newline convention.
        data = path.read_bytes().replace(b"\r\n", b"\n")
        actual = hashlib.sha256(data).hexdigest()
        if actual != expected:
            raise ValueError(f"projection input changed at {path}. Expected SHA-256 {expected}, got {actual}")
        sources[name] = data.decode("utf-8")
    return sources


def between(text, start, end):
    first = text.index(start)
    return text[first:text.index(end, first + len(start))]


def rust_function(text, start):
    return between(text, start, "\n}\n") + "\n}\n"


def generate(root, sources):
    (root / "Cargo.toml").write_text(MANIFEST, encoding="utf-8")
    (root / "Cargo.lock").write_text(LOCK, encoding="utf-8")
    src = root / "src"
    src.mkdir()
    fixture = Path(__file__).with_name("fixture.rs")
    (src / "lib.rs").write_bytes(fixture.read_bytes())

    camera = sources["camera"]
    jmath = sources["jmath"]
    (src / "camera.rs").write_text(
        "const PI: f32 = std::f32::consts::PI;\n"
        + between(sources["trig"], "pub const DEG_TO_RAD:", "\npub const RAD_TO_DEG:")
        + "\n" + rust_function(camera, "pub mod joml {")
        + "use joml::Quat;\n" + rust_function(camera, "pub fn view_matrix(")
        + "use crate::horizon_core;\n" + rust_function(camera, "pub fn perspective("),
        encoding="utf-8",
    )
    (src / "jmath.rs").write_text(
        between(jmath, "#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub enum Platform", "/// Java `sin`.")
        + rust_function(jmath, "pub fn tan("),
        encoding="utf-8",
    )
    (src / "terrain.rs").write_text(
        "use super::TerrainVertex;\n"
        + between(
            sources["terrain"],
            "#[derive(Clone, Copy, Debug)]\npub struct TerrainBounds",
            "pub fn terrain_clip_rows(",
        ),
        encoding="utf-8",
    )
    (src / "hands.rs").write_text(
        "use super::{HandFrame, camera, terrain::{terrain_view_rows, TerrainClipRows}};\n\n"
        "pub(super) fn hand_clip(frame: &HandFrame, aspect: f32) -> TerrainClipRows {\n"
        + between(sources["hands"], "        let view = frame.bob", "        self.draw(")
        + "    projection\n}\n",
        encoding="utf-8",
    )
    (src / "outline.rs").write_text(
        sources["outline"] + "\n" + rust_function(sources["gfx"], "pub fn outline_width("),
        encoding="utf-8",
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("assembled_source", type=Path, help="verified assembled RC46 source checkout or export")
    parser.add_argument("--offline", action="store_true", help="use Cargo's existing dependency cache")
    args = parser.parse_args()
    try:
        root = args.assembled_source.resolve(strict=True)
        sources = read_inputs(root)
        print(f"Verified {len(sources)} RC46 projection inputs at {root}", flush=True)
        with tempfile.TemporaryDirectory(prefix="horizon-first-person-crop-") as scratch:
            build = Path(scratch)
            generate(build, sources)
            command = [os.environ.get("CARGO", "cargo"), "test", "--locked"]
            if args.offline:
                command.append("--offline")
            env = os.environ | {"CARGO_TARGET_DIR": str(build / "target")}
            return subprocess.run(command, cwd=build, env=env).returncode
    except (OSError, ValueError) as error:
        print(f"Projection fixture failed. {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
