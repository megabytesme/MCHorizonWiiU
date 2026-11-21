import copy
import json
from pathlib import Path
import tempfile
import tomllib
import unittest

import core
import native
import nightly
import source


class CatalogTests(unittest.TestCase):
    def setUp(self):
        self.recipe, self.fingerprint = source.read_recipe(source.MANIFEST)

    def test_checked_in_recipe_inputs(self):
        self.assertEqual(self.fingerprint, core.file_digest(source.MANIFEST))
        lock = source.checked_bytes(core.ROOT, native.INPUTS_FILE, self.recipe["inputs_sha256"])
        self.assertEqual(core.parse_json(lock)["version"], self.recipe["toolchain"])
        for item in source.native_files(self.recipe, core.ROOT):
            if "source" in item:
                source.checked_bytes(core.ROOT, item["source"], item["sha256"])
        folder = core.ROOT / "patches" / self.recipe["upstream"]["commit"]
        for item in [*self.recipe["series"], self.recipe["diagnostic"]]:
            source.checked_bytes(folder, item["file"], item["sha256"])

    def test_hosted_recipe_uses_the_standalone_pin_and_exact_inputs(self):
        spec, fingerprint, files = native.hosted_inputs(self.recipe, self.fingerprint)
        self.assertEqual(fingerprint, core.file_digest(core.ROOT / native.HOSTED_SPEC))
        self.assertNotEqual(spec["input_tree"], spec["output_tree"])
        self.assertTrue(all(files))
        with self.assertRaisesRegex(ValueError, "standalone pin"):
            native.hosted_inputs(self.recipe, "0" * 64)

    def test_nightly_pins_all_patches_and_keeps_its_own_tree_chain(self):
        spec, fingerprint, patches = nightly.read_recipe(self.fingerprint)
        self.assertEqual(core.file_digest(nightly.MANIFEST), fingerprint)
        for patch in spec["series"]:
            self.assertEqual(core.digest(patches[patch["file"]]), patch["sha256"])
        with self.assertRaisesRegex(ValueError, "toolchain recipe"):
            nightly.read_recipe("0" * 64)
        broken = copy.deepcopy(spec)
        broken["series"][1]["input_tree"] = "f" * 40
        with self.assertRaisesRegex(ValueError, "noncontiguous"):
            nightly.validate(broken)

    def test_reject_duplicate_fields_broken_chain_and_wrong_versions(self):
        with self.assertRaisesRegex(ValueError, "duplicate"):
            core.parse_json('{"native":{"tree":1,"tree":2}}')
        for field, value in (("schema", True), ("builder", "9.0.0"), ("target", "x64")):
            with self.subTest(field=field), self.assertRaises(ValueError):
                source.validate_recipe(dict(self.recipe, **{field: value}))
        broken = copy.deepcopy(self.recipe)
        broken["series"][1]["from"] = "f" * 40
        with self.assertRaisesRegex(ValueError, "noncontiguous"):
            source.validate_recipe(broken)

    def test_path_traversal_is_rejected(self):
        for path in ("../data", "path/../data", "C:/file", "/file", "a/.git/index", "a//b"):
            with self.subTest(path=path), self.assertRaises(ValueError):
                source.relative(path)


def git_out(root, *args):
    return core.git(root, *args).stdout


def commit(root, message):
    git_out(root, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
            "commit", "--quiet", "-m", message)
    return git_out(root, "rev-parse", "HEAD").decode().strip()


class AssemblyTests(unittest.TestCase):
    def test_materializes_exact_tree_and_applies_opt_in_diagnostic(self):
        with tempfile.TemporaryDirectory() as temp:
            temp = Path(temp)
            upstream = temp / "horizon"
            port = temp / "port"
            upstream.mkdir()
            port.mkdir()
            git_out(upstream, "init", "--quiet", "--template=")
            game_file = upstream / "game"
            game_file.write_bytes(b"upstream\n")
            git_out(upstream, "add", "game")
            base = commit(upstream, "upstream")
            before = git_out(upstream, "rev-parse", f"{base}^{{tree}}").decode().strip()
            inventory = core.digest(git_out(upstream, "ls-tree", "-rz", "--full-tree", base))
            game_file.write_bytes(b"patched game\n")
            native_path = upstream / "wiiu"
            native_path.mkdir()
            (native_path / "client").write_bytes(b"native client\n")
            git_out(upstream, "add", "game", "wiiu/client")
            finished = commit(upstream, "patched and overlaid")
            tree = git_out(upstream, "rev-parse", f"{finished}^{{tree}}").decode().strip()
            delta = git_out(upstream, "diff", "--binary", base, finished, "--", "game")
            game_file.write_bytes(b"diagnostic game\n")
            git_out(upstream, "add", "game")
            diag = commit(upstream, "test instrumentation")
            diag_tree = git_out(upstream, "rev-parse", f"{diag}^{{tree}}").decode().strip()
            diag_delta = git_out(upstream, "diff", "--binary", finished, diag)
            paths = port / "patches" / base
            (paths / "diagnostics").mkdir(parents=True)
            (paths / "0001-game.patch").write_bytes(delta)
            (paths / "diagnostics/0001-logs.patch").write_bytes(diag_delta)
            (port / "client").write_bytes(b"native client\n")
            native_manifest = json.dumps([{
                "source": "client", "destination": "wiiu/client", "mode": "100644",
                "sha256": core.digest(b"native client\n"),
            }]).encode()
            (port / "catalog").mkdir()
            (port / "catalog/native-files-v1.json").write_bytes(native_manifest)
            pinned = {"commit": base, "tree": before, "inventory_sha256": inventory}
            recipe = dict(source.read_recipe(source.MANIFEST)[0], upstream=pinned,
                          native={"commit": finished, "tree": tree,
                                  "inventory_sha256": core.digest(native_manifest)},
                          series=[{"file": "0001-game.patch", "from": base, "to": finished,
                                   "sha256": core.digest(delta)}],
                          diagnostic={"file": "diagnostics/0001-logs.patch", "from": finished,
                                      "to": diag, "tree": diag_tree, "sha256": core.digest(diag_delta)})
            source.validate_recipe(recipe)
            with self.assertRaisesRegex(ValueError, "exact pin"):
                source.assemble(upstream, "HEAD", recipe, "fixture", temp / "wrong", root=port)
            stable = temp / "assembled"
            source.assemble(upstream, base, recipe, "fixture", stable, root=port)
            # Another process may refresh the assembly index while this build
            # verifies its contents. Verification only needs to read it.
            shared_index_lock = stable / "source/.git/index.lock"
            shared_index_lock.touch()
            try:
                assembled = source.verify_assembly(stable, recipe, "fixture")
            finally:
                shared_index_lock.unlink()
            self.assertEqual((assembled / "game").read_bytes(), b"patched game\n")
            self.assertEqual((assembled / "wiiu/client").read_bytes(), b"native client\n")
            with self.assertRaisesRegex(ValueError, "already exists"):
                source.assemble(upstream, base, recipe, "fixture", stable, root=port)
            debug = temp / "diagnostic"
            source.assemble(upstream, base, recipe, "fixture", debug, True, port)
            self.assertEqual((debug / "source/game").read_bytes(), b"diagnostic game\n")
            with self.assertRaisesRegex(ValueError, "stable game recipe"):
                source.verify_assembly(debug, recipe, "fixture")
            (assembled / "game").write_bytes(b"corrupted after assembly\n")
            with self.assertRaisesRegex(ValueError, "verified index"):
                source.verify_assembly(stable, recipe, "fixture")


class CargoInputsTests(unittest.TestCase):
    def test_local_packages_use_the_same_relative_paths_in_two_build_roots(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            inputs = root / "inputs"
            for name in native.OVERLAYS:
                crate = inputs / name
                crate.mkdir(parents=True)
                (crate / "Cargo.toml").write_text(f'[package]\nname = "{name}"\n')
            lock = {"overlay_sha256": {name: native.inventory_digest(inputs / name)
                                       for name in native.OVERLAYS}}
            bridge = {"lock": b"Cargo lock fixture", "tv_size": b"native link fixture"}
            manifests = []
            for name in ("first", "second"):
                build = root / name
                staged = build / "source"
                loading_app = staged / "wiiu/loading_app"
                loading_app.mkdir(parents=True)
                git_out(staged, "init", "--quiet", "--template=")
                (loading_app / "Cargo.toml").write_text(
                    '[lib]\n\n[workspace]\n\n[dependencies]\n'
                    'horizon-client = { path = "../../crates/horizon-client" }\n'
                    '\n[patch.crates-io]\n')
                client = staged / "crates/horizon-client"
                client.mkdir(parents=True)
                (client / "Cargo.toml").write_text('[package]\nname = "horizon-client"\n')
                git_out(staged, "add", "wiiu/loading_app/Cargo.toml", "crates/horizon-client/Cargo.toml")
                manifest = native.write_cargo_inputs(build, staged, inputs, lock, bridge)
                self.assertTrue(manifest.parent.samefile(build))
                manifests.append(manifest.read_bytes())
            self.assertEqual(*manifests)
            resolved = tomllib.loads(manifests[0].decode())
            self.assertEqual(resolved["lib"]["path"], "source/wiiu/loading_app/src/lib.rs")
            self.assertEqual(resolved["dependencies"]["horizon-client"]["path"], "source/crates/horizon-client")
            self.assertEqual(resolved["patch"]["crates-io"]["winit"]["path"], "external/winit")
            self.assertIn("source/crates/horizon-client", resolved["workspace"]["exclude"])


class OutputTests(unittest.TestCase):
    def test_checks_sizes_and_hashes_of_both_game_outputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in native.GAME_PACKAGES:
                (root / name).write_bytes(b"a")
            files = {name: {"bytes": 1, "sha256":
                     "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb"}
                     for name in native.GAME_PACKAGES}
            record = {"schema": 1, "kind": "wiiu-native-game-build", "outputs": files}
            core.verify_outputs(record, root)
            (root / native.GAME_PACKAGES[0]).write_bytes(b"b")
            with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
                core.verify_outputs(record, root)

    def test_verifies_only_the_combined_pair_for_a_hosted_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in native.HOSTED_PACKAGES:
                (root / name).write_bytes(b"combined fixture")
            outputs = core.artifacts(root, native.HOSTED_PACKAGES)
            record = {"schema": 1, "kind": "wiiu-hosted-game-build", "outputs": outputs}
            core.verify_outputs(record, root)
            with self.assertRaises(ValueError):
                core.verify_outputs(dict(record, kind="wiiu-native-game-build"), root)

    def test_reuses_resource_inventory_without_copying_and_detects_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            prepared = root / "prepared"
            prepared.mkdir()
            (prepared / "asset").write_bytes(b"a")
            manifest = core.inspect_resources(prepared, root / "private.json")
            self.assertEqual(native.verify_resources(prepared, manifest), core.file_digest(manifest))
            self.assertEqual({path.resolve() for path in root.iterdir()}, {prepared.resolve(), manifest})
            (prepared / "asset").write_bytes(b"b")
            with self.assertRaisesRegex(ValueError, "private inventory"):
                native.verify_resources(prepared, manifest)


if __name__ == "__main__":
    unittest.main()
