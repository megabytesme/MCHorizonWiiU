import json
from pathlib import Path
import tempfile
import unittest

from tools import rebind_cafe_prepared_archive as archive


GUEST_ROOT = "/vol/external01/wiiu/apps/HorizonWiiU/instances/Qualification/.local/horizon-client/26.3"
PIN = {
    "minecraft": "26.3",
    "schema": 4,
    "client_sha256": "1" * 64,
    "server_sha256": "2" * 64,
    "exporter_sha256": "3" * 64,
}
ASSET = b"assets/example/lang/en_us.json"
DATA = b"{}"


class ArchivePreflightTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.preparation = Path(temporary.name)
        self.recipe = self.preparation / "recipe.json"
        self.recipe.write_text(json.dumps({"resource_pin": PIN}), encoding="utf-8")
        self.manifest = json.dumps(PIN).encode()
        (self.preparation / archive.MANIFEST).write_bytes(self.manifest)
        self.source = self.preparation / archive.ARCHIVE
        self.destination = self.preparation / "bound.bin"
        self.write_source()
        archive.rebind(self.preparation, self.destination, GUEST_ROOT)

    def write_source(self, *, offset_shift=0):
        guest = archive.DEFAULT_ROOT.encode()
        header = archive.HEADER.pack(
            archive.MAGIC, 2, 1, len(guest), len(self.manifest), len(ASSET),
        )

        def pad(buf):
            return buf.ljust(archive.padded(len(buf)), b"\0")

        prefix = header + pad(guest) + pad(self.manifest)
        payload_at = len(prefix) + archive.ENTRY.size + archive.padded(len(ASSET))
        entry = archive.ENTRY.pack(0, len(ASSET), payload_at + offset_shift, len(DATA))
        self.source.write_bytes(prefix + entry + pad(ASSET) + DATA)

    def check(self, path, **kwargs):
        return archive.preflight(self.preparation, path, GUEST_ROOT, self.recipe, **kwargs)

    def test_rejects_default_root_for_an_isolated_instance(self):
        with self.assertRaisesRegex(ValueError, "root mismatch.*expected.*found"):
            self.check(self.source)
        checked = self.check(self.destination, payload_from=self.source)
        self.assertEqual(checked["guest_root"], GUEST_ROOT)
        self.assertEqual(checked["entries"], 1)
        self.assertEqual(
            self.check(self.destination, expected_sha256=checked["archive_sha256"]), checked,
        )

    def test_detects_mutated_content_and_fingerprint(self):
        checked = self.check(self.destination)
        with self.destination.open("r+b") as changed:
            changed.seek(-1, 2)
            changed.write(b"]")
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            self.check(self.destination, expected_sha256=checked["archive_sha256"])
        with self.assertRaisesRegex(ValueError, "entries and payload"):
            self.check(self.destination, payload_from=self.source)

    def test_refuses_a_different_recipe_and_a_different_embedded_manifest(self):
        self.recipe.write_text(json.dumps({"resource_pin": dict(PIN, schema=3)}))
        with self.assertRaisesRegex(ValueError, "schema and exporter pins"):
            self.check(self.destination)
        self.recipe.write_text(json.dumps({"resource_pin": PIN}))
        manifest_path = self.preparation / archive.MANIFEST
        manifest_path.write_bytes(self.manifest.replace(b'"schema": 4', b'"schema":\t4'))
        with self.assertRaisesRegex(ValueError, "different manifest"):
            self.check(self.destination)

    def test_refuses_an_entry_outside_the_archive(self):
        self.write_source(offset_shift=1)
        with self.assertRaisesRegex(ValueError, "out-of-range entry"):
            self.check(self.destination, payload_from=self.source)
