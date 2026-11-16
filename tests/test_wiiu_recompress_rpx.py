import binascii
import importlib.util
from pathlib import Path
import struct
import sys
import tempfile
import unittest
import zlib


TOOL = Path(__file__).resolve().parents[1] / "tools" / "wiiu_recompress_rpx.py"
SPEC = importlib.util.spec_from_file_location("wiiu_recompress_rpx", TOOL)
recompressor = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = recompressor
SPEC.loader.exec_module(recompressor)


def compressed_section(raw):
    return len(raw).to_bytes(4, "big") + zlib.compress(raw, 6)


def synthetic_rpx(*, rela_count=1000, overlap=False, corrupt_rela=False, bad_rela_size=False,
                  bad_crc=False, truncated_rela=False, trailing_rela=False,
                  bad_rela_prefix=False):
    rela_raw = b"".join(struct.pack(">III", index * 4, 0x17, 0) for index in range(rela_count))
    text_raw = b"Cafe RPX synthetic text section\0" * 8
    fileinfo_raw = struct.pack(">I", recompressor.RPL_FILEINFO_MAGIC) + bytes(92)
    strings = b"\0.text\0.rela.text\0.shstrtab\0.rpl.crcs\0.rpl.fileinfo\0"
    names = {
        ".text": strings.index(b".text"),
        ".rela.text": strings.index(b".rela.text"),
        ".shstrtab": strings.index(b".shstrtab"),
    }
    text_stored = compressed_section(text_raw)
    rela_stored = compressed_section(rela_raw)
    if corrupt_rela:
        rela_stored = rela_stored[:4] + b"\x00\x00\x00\x00" + rela_stored[8:]
    if truncated_rela:
        rela_stored = rela_stored[:-1]
    if trailing_rela:
        rela_stored += b"trailing"
    if bad_rela_size:
        rela_stored = (0xFFFFFFFF).to_bytes(4, "big") + rela_stored[4:]
    if bad_rela_prefix:
        rela_stored = (len(rela_raw) - 1).to_bytes(4, "big") + rela_stored[4:]
    names_stored = compressed_section(strings)

    section_count = 6
    section_table_offset = 64
    section_table_end = section_table_offset + section_count * recompressor.ELF32_SECTION_HEADER_SIZE
    text_offset = (section_table_end + 3) & ~3
    rela_offset = text_offset + len(text_stored)
    if overlap:
        rela_offset = text_offset + len(text_stored) - 1
    names_offset = rela_offset + len(rela_stored)
    crcs_offset = names_offset + len(names_stored)
    fileinfo_offset = crcs_offset + section_count * 4
    file_size = fileinfo_offset + len(fileinfo_raw)
    data = bytearray(file_size)

    ident = bytearray(16)
    ident[:9] = b"\x7fELF\x01\x02\x01\xca\xfe"
    struct.pack_into(">HHIIIIIHHHHHH", data, 16,
                     recompressor.ELF_TYPE_RPX, recompressor.ELF_MACHINE_PPC,
                     recompressor.ELF_VERSION_CURRENT, 0x02000000, 0, section_table_offset,
                     0, recompressor.ELF_HEADER_SIZE, 0, 0,
                     recompressor.ELF32_SECTION_HEADER_SIZE, section_count, 3)
    data[:16] = ident
    data[text_offset : text_offset + len(text_stored)] = text_stored
    data[rela_offset : rela_offset + len(rela_stored)] = rela_stored
    data[names_offset : names_offset + len(names_stored)] = names_stored
    data[fileinfo_offset : fileinfo_offset + len(fileinfo_raw)] = fileinfo_raw

    sections = [
        (0, recompressor.SHT_NULL, 0, 0, 0, 0, 0, 0, 0, 0),
        (names[".text"], 1, recompressor.SHF_RPL_ZLIB_COMPRESSED | 0x6,
         0x02000000, text_offset, len(text_stored), 0, 0, 4, 0),
        (names[".rela.text"], recompressor.SHT_RELA,
         recompressor.SHF_RPL_ZLIB_COMPRESSED, 0, rela_offset,
         len(rela_stored), 0, 1, 4, recompressor.ELF32_RELA_SIZE),
        (names[".shstrtab"], recompressor.SHT_STRTAB,
         recompressor.SHF_RPL_ZLIB_COMPRESSED | 0x2, 0,
         names_offset, len(names_stored), 0, 0, 1, 0),
        (0, recompressor.SHT_RPL_CRCS, 0, 0, crcs_offset, section_count * 4, 0, 0, 4, 4),
        (0, recompressor.SHT_RPL_FILEINFO, 0, 0, fileinfo_offset, len(fileinfo_raw), 0, 0, 4, 0),
    ]
    for index, fields in enumerate(sections):
        struct.pack_into(">IIIIIIIIII", data,
                         section_table_offset + index * recompressor.ELF32_SECTION_HEADER_SIZE,
                         *fields)

    crc_values = [0] * section_count
    crc_values[1] = binascii.crc32(text_raw) & 0xFFFFFFFF
    crc_values[2] = binascii.crc32(rela_raw) & 0xFFFFFFFF
    crc_values[3] = binascii.crc32(strings) & 0xFFFFFFFF
    crc_values[5] = binascii.crc32(fileinfo_raw) & 0xFFFFFFFF
    if bad_crc:
        crc_values[1] ^= 1
    struct.pack_into(">6I", data, crcs_offset, *crc_values)
    return bytes(data)


class RecompressRpxTests(unittest.TestCase):
    def test_recompresses_only_compressed_rela_and_preserves_invariants(self):
        source = synthetic_rpx()
        image = recompressor._parse_rpx(source)
        original_rela = next(section for section in image.sections if section.kind == recompressor.SHT_RELA)
        original_raw = recompressor._raw_section(source, original_rela)
        result = recompressor.recompress_rpx(source)
        updated = recompressor._parse_rpx(result.data)
        updated_rela = updated.sections[original_rela.index]

        self.assertEqual(result.sections_recompressed, 1)
        self.assertGreater(result.bytes_saved, 0)
        self.assertEqual(len(result.data), len(source))
        self.assertEqual(updated_rela.offset, original_rela.offset)
        self.assertLess(updated_rela.size, original_rela.size)
        self.assertEqual(recompressor._raw_section(result.data, updated_rela), original_raw)
        self.assertEqual(
            recompressor._stored_section(result.data, updated.sections[updated.crc_section_index]),
            recompressor._stored_section(source, image.sections[image.crc_section_index]),
        )
        self.assertEqual(
            recompressor._stored_section(result.data, updated.sections[updated.fileinfo_section_index]),
            recompressor._stored_section(source, image.sections[image.fileinfo_section_index]),
        )
        changed_size_headers = {
            original_rela.header_offset + 20,
        }
        for offset, (before, after) in enumerate(zip(source, result.data, strict=True)):
            if before != after:
                self.assertTrue(
                    original_rela.offset <= offset < original_rela.offset + original_rela.size
                    or offset in range(original_rela.header_offset + 20, original_rela.header_offset + 24),
                    f"unexpected changed byte at {offset:#x}",
                )
        self.assertEqual(len(changed_size_headers), 1)
        tail_start = original_rela.offset + updated_rela.size
        tail_end = original_rela.offset + original_rela.size
        self.assertEqual(result.data[tail_start:tail_end], bytes(tail_end - tail_start))

    def test_no_savings_fails_without_creating_output_or_changing_input(self):
        source_data = synthetic_rpx(rela_count=1)
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "source.rpx"
            output = Path(directory) / "output.rpx"
            source.write_bytes(source_data)
            with self.assertRaisesRegex(recompressor.RpxError, "no RELA section size savings"):
                recompressor.recompress_file(source, output)
            self.assertEqual(source.read_bytes(), source_data)
            self.assertFalse(output.exists())

    def test_malformed_compressed_stream_fails_without_publishing(self):
        source_data = synthetic_rpx(corrupt_rela=True)
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "broken.rpx"
            output = Path(directory) / "output.rpx"
            source.write_bytes(source_data)
            with self.assertRaises(recompressor.RpxError):
                recompressor.recompress_file(source, output)
            self.assertEqual(source.read_bytes(), source_data)
            self.assertFalse(output.exists())

    def test_truncated_stream_fails_without_publishing(self):
        self._assert_invalid_rpx_not_published(synthetic_rpx(truncated_rela=True))

    def test_trailing_stream_data_fails_without_publishing(self):
        self._assert_invalid_rpx_not_published(synthetic_rpx(trailing_rela=True))

    def test_declared_raw_size_mismatch_fails_without_publishing(self):
        self._assert_invalid_rpx_not_published(synthetic_rpx(bad_rela_prefix=True))

    def test_crc_mismatch_fails_without_publishing(self):
        self._assert_invalid_rpx_not_published(synthetic_rpx(bad_crc=True))

    def _assert_invalid_rpx_not_published(self, source_data):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "invalid.rpx"
            output = Path(directory) / "output.rpx"
            source.write_bytes(source_data)
            with self.assertRaises(recompressor.RpxError):
                recompressor.recompress_file(source, output)
            self.assertEqual(source.read_bytes(), source_data)
            self.assertFalse(output.exists())

    def test_unreasonable_compressed_size_fails_without_publishing(self):
        source_data = synthetic_rpx(bad_rela_size=True)
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "oversized.rpx"
            output = Path(directory) / "output.rpx"
            source.write_bytes(source_data)
            with self.assertRaisesRegex(recompressor.RpxError, "safety limit"):
                recompressor.recompress_file(source, output)
            self.assertEqual(source.read_bytes(), source_data)
            self.assertFalse(output.exists())

    def test_oversized_sparse_input_is_rejected_before_publication(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "oversized.rpx"
            output = Path(directory) / "output.rpx"
            with source.open("wb") as source_file:
                source_file.truncate(recompressor.MAX_FILE_SIZE + 1)
            original_size = source.stat().st_size
            with self.assertRaisesRegex(recompressor.RpxError, "file-size safety limit"):
                recompressor.recompress_file(source, output)
            self.assertEqual(source.stat().st_size, original_size)
            self.assertFalse(output.exists())

    def test_overlapping_sections_fail_without_publishing(self):
        source_data = synthetic_rpx(overlap=True)
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "overlap.rpx"
            output = Path(directory) / "output.rpx"
            source.write_bytes(source_data)
            with self.assertRaisesRegex(recompressor.RpxError, "overlap"):
                recompressor.recompress_file(source, output)
            self.assertEqual(source.read_bytes(), source_data)
            self.assertFalse(output.exists())

    def test_existing_output_is_never_overwritten(self):
        source_data = synthetic_rpx()
        existing_output = b"keep existing destination"
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "source.rpx"
            output = Path(directory) / "output.rpx"
            source.write_bytes(source_data)
            output.write_bytes(existing_output)
            with self.assertRaisesRegex(recompressor.RpxError, "already exists"):
                recompressor.recompress_file(source, output)
            self.assertEqual(source.read_bytes(), source_data)
            self.assertEqual(output.read_bytes(), existing_output)

    def test_dangling_output_symlink_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "source.rpx"
            output = Path(directory) / "output.rpx"
            target = Path(directory) / "missing-target.rpx"
            source.write_bytes(synthetic_rpx())
            try:
                output.symlink_to(target)
            except OSError as error:
                self.skipTest(f"symlink creation unavailable: {error}")
            with self.assertRaisesRegex(recompressor.RpxError, "already exists"):
                recompressor.recompress_file(source, output)
            self.assertTrue(output.is_symlink())
            self.assertFalse(target.exists())

    def test_same_input_and_output_path_is_rejected(self):
        source_data = synthetic_rpx()
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "source.rpx"
            source.write_bytes(source_data)
            with self.assertRaisesRegex(recompressor.RpxError, "must be distinct"):
                recompressor.recompress_file(source, source)
            self.assertEqual(source.read_bytes(), source_data)

    def test_successful_file_output_is_published_without_changing_source(self):
        source_data = synthetic_rpx()
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "source.rpx"
            output = Path(directory) / "output.rpx"
            source.write_bytes(source_data)
            result = recompressor.recompress_file(source, output)
            self.assertEqual(source.read_bytes(), source_data)
            self.assertEqual(output.read_bytes(), result.data)
            self.assertGreater(result.bytes_saved, 0)


if __name__ == "__main__":
    unittest.main()
