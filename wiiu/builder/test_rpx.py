import tempfile
import unittest
from pathlib import Path
from struct import pack
from zlib import compress

from rpx import read_code_heap, require_aroma_fit


def fixture(expanded, declared):
    ident = b"\x7fELF\x01\x02\x01\xca\xfe".ljust(16, b"\0")
    header = pack(">16sHHIIIIIHHHHHH", ident, 0xFE01, 20, 1, 0x02000000, 0, 52,
                  0, 52, 0, 0, 40, 3, 0)
    text = pack(">I", expanded) + compress(bytes(expanded))
    info = pack(">24I", 0xCAFE0402, declared, 32, *([0] * 21))
    sections = (
        bytes(40),
        pack(">10I", 0, 1, 0x08000006, 0x02000000, 172, len(text), 0, 0, 4, 0),
        pack(">10I", 0, 0x80000004, 0, 0, 172 + len(text), len(info), 0, 0, 4, 0),
    )
    return header + b"".join(sections) + text + info


class AromaCodeHeapTests(unittest.TestCase):
    def inspect(self, data):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "fixture.rpx"
            path.write_bytes(data)
            return read_code_heap(path)

    def test_reads_expansion_and_allocation_padding(self):
        report = self.inspect(fixture(1030, 1056))
        self.assertEqual(report["expanded_executable_bytes"], 1030)
        self.assertLess(report["code_sections"][0]["stored_bytes"], 1030)
        self.assertEqual(report["code_padding_bytes"], 26)
        self.assertEqual(report["estimated_text_allocation_bytes"], 1152)
        require_aroma_fit(report)

    def test_rejects_inconsistent_text_declaration(self):
        with self.assertRaisesRegex(ValueError, "code span disagrees"):
            self.inspect(fixture(1030, 1024))

    def test_rejects_hardware_overrun(self):
        with self.assertRaisesRegex(ValueError, "by 974,016 bytes"):
            require_aroma_fit({"remaining_bytes": -974016})

    def test_rejects_truncated_or_non_cafe_input(self):
        with self.assertRaisesRegex(ValueError, "truncated"):
            self.inspect(fixture(1030, 1056)[:-10])
        with self.assertRaisesRegex(ValueError, "PowerPC Cafe RPX"):
            self.inspect(bytes(52))
