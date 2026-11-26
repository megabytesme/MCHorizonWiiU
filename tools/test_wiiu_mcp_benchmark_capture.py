import base64
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from gx2_profile import HEADER, V3_ROW
from wiiu_mcp_benchmark_capture import CHUNK_BYTES, download, publish


CAPTURE_ID = "game-20261126200000-cafe-stationary-4q.hzp"


def fixture_capture():
    count = CHUNK_BYTES // V3_ROW.size + 1
    header = HEADER.pack(0x485A5033, 3, 27_000_000, V3_ROW.size, 4096, count, count)
    tail = [0] * (V3_ROW.size // 4 - 6)
    return header + b"".join(V3_ROW.pack(seq, 5, 0, seq * 100, 100, 100, *tail) for seq in range(count))


class CaptureTests(unittest.TestCase):
    def service(self, corrupt=None):
        capture = fixture_capture()
        offsets = []

        def call(tool, args):
            self.assertEqual(tool, "wiiu_benchmark_capture")
            self.assertEqual((args["instance"], args["capture_id"]), ("fixture", CAPTURE_ID))
            offset = args["offset"]
            offsets.append(offset)
            block = capture[offset:offset + CHUNK_BYTES]
            end = offset + len(block)
            reply = {"capture_id": CAPTURE_ID, "instance": "fixture", "format": "HZP3", "encoding": "base64",
                     "offset": offset, "total_bytes": len(capture), "next_offset": end if end < len(capture) else None,
                     "data": base64.b64encode(block).decode("ascii")}
            if corrupt:
                corrupt(reply)
            return {"content": [{"type": "text", "text": json.dumps(reply)}]}

        return capture, offsets, call

    def test_reconstructs_a_two_page_profile_exactly(self):
        expected, offsets, call = self.service()
        capture, metadata = download(call, "fixture", CAPTURE_ID)
        self.assertEqual(offsets, [0, CHUNK_BYTES])
        self.assertEqual(capture, expected)
        self.assertEqual(metadata["version"], 3)

    def test_rejects_progress_that_does_not_match_the_page(self):
        _, offsets, call = self.service(lambda reply: reply.update(next_offset=1))
        with self.assertRaisesRegex(ValueError, "next capture offset"):
            download(call, "fixture", CAPTURE_ID)
        self.assertEqual(offsets, [0])

    def test_publishes_only_to_a_new_file(self):
        capture = fixture_capture()
        with TemporaryDirectory() as directory:
            target = Path(directory) / CAPTURE_ID
            publish(target, capture)
            with self.assertRaises(FileExistsError):
                publish(target, b"replacement")
            self.assertEqual(target.read_bytes(), capture)
            self.assertEqual(list(Path(directory).iterdir()), [target])


if __name__ == "__main__":
    unittest.main()
