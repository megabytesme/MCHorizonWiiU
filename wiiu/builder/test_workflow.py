import copy
from datetime import datetime, timezone
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import core
import dependencies
import releases
import workflow


class ReleaseSelectionTests(unittest.TestCase):
    def setUp(self):
        self.catalog, self.payload = releases.read_catalog()
        self.args = {"builder": "0.6.0", "client": "0.7.0", "patch": "0.4.0",
                     "toolchain": "1.0.0", "channel": "development"}

    def test_exact_compatible_tuple_and_revocation(self):
        rule = releases.resolve(self.catalog, **self.args)
        self.assertEqual(rule["upstream"], "69fab597a6f0e8a6ce524d38d1b73e599e343034")
        for field, value in (("client", "0.6.1"), ("patch", "0.3.0"), ("channel", "nightly"),
                             ("toolchain", "2.0.0"), ("builder", "0.4.0"), ("target", "x86_64")):
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "compatible"):
                releases.resolve(self.catalog, **dict(self.args, **{field: value}))
        revoked = copy.deepcopy(self.catalog)
        revoked["releases"]["client"]["0.7.0"]["status"] = "revoked"
        with self.assertRaisesRegex(ValueError, "revoked"):
            releases.resolve(revoked, **self.args)

    def test_rejects_rollback_sequence_reuse_and_expiry(self):
        with tempfile.TemporaryDirectory() as scratch:
            state = Path(scratch) / "trust.json"
            now = datetime(2026, 11, 21, tzinfo=timezone.utc)
            newer = dict(self.catalog, sequence=self.catalog["sequence"] + 1)
            releases.check_sequence(newer, b"new catalog", state, now)
            releases.check_sequence(newer, b"new catalog", state, now)
            with self.assertRaisesRegex(ValueError, "sequence reuse"):
                releases.check_sequence(newer, b"different", state, now)
            with self.assertRaisesRegex(ValueError, "rollback"):
                releases.check_sequence(self.catalog, self.payload, state, now)
            with self.assertRaisesRegex(ValueError, "validity interval"):
                releases.check_sequence(newer, b"new catalog", state, datetime(2100, 1, 1, tzinfo=timezone.utc))

    def test_rejects_wrong_abi_before_resolution(self):
        bad = copy.deepcopy(self.catalog)
        bad["compatibility"][0]["bridge_abi"] = 99
        with tempfile.TemporaryDirectory() as scratch:
            catalog = Path(scratch) / "catalog.json"
            core.write_record(catalog, bad)
            with self.assertRaisesRegex(ValueError, "ABI mismatch"):
                releases.read_catalog(catalog)


class CacheTests(unittest.TestCase):
    def test_reject_low_space_before_creating_output(self):
        with tempfile.TemporaryDirectory() as scratch:
            target = Path(scratch) / "build"
            with patch.object(workflow.shutil, "disk_usage", return_value=type("Usage", (), {"free": 32})()):
                with self.assertRaisesRegex(ValueError, "needs at least"):
                    workflow.require_space(target, workflow.MIN_BUILD_SPACE)
            self.assertFalse(target.exists())

    def test_incomplete_download_preserves_existing_file(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            data = b"new trusted archive"
            name = "ring-0.17.14.crate"
            url = f"https://static.crates.io/crates/ring/{name}"
            path = root / name
            path.write_bytes(b"damaged")
            record = {"name": name, "url": url, "sha256": core.digest(data)}

            def interrupted(_):
                raise InterruptedError("network stopped")

            class Response(io.BytesIO):
                def geturl(self):
                    return url

            response = Response(data)
            response.read = interrupted
            with patch.object(dependencies, "urlopen", return_value=response), self.assertRaises(InterruptedError):
                dependencies.get_archive(record, root, offline=False)
            self.assertEqual(path.read_bytes(), b"damaged")
            self.assertEqual(list(root.iterdir()), [path])
            with self.assertRaisesRegex(ValueError, "SHA-256"):
                dependencies.get_archive(record, root, offline=True)
            with patch.object(dependencies, "urlopen", return_value=Response(data)):
                dependencies.get_archive(record, root, offline=False)
            with patch.object(dependencies, "urlopen", side_effect=AssertionError("offline network access")):
                self.assertEqual(dependencies.get_archive(record, root, offline=True).read_bytes(), data)

    def test_an_archive_path_cannot_escape_the_cache(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            archive = root / "demo-1.0.0.crate"
            with tarfile.open(archive, "w:gz") as stream:
                member = tarfile.TarInfo("demo-1.0.0/../../outside")
                member.size = 3
                stream.addfile(member, io.BytesIO(b"bad"))
            output = root / "inside"
            output.mkdir()
            with self.assertRaisesRegex(ValueError, "unsafe relative"):
                dependencies.extract_archive(archive, output)
            self.assertFalse((root / "outside").exists())
