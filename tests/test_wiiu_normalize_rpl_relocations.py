import importlib.util
from pathlib import Path
import struct
import sys
import tempfile
import unittest


TOOL = Path(__file__).resolve().parents[1] / "tools" / "wiiu_normalize_rpl_relocations.py"
SPEC = importlib.util.spec_from_file_location("wiiu_normalize_rpl_relocations", TOOL)
normalizer = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = normalizer
SPEC.loader.exec_module(normalizer)


def synthetic_local24pc_elf(
    *, visibility, binding=normalizer.STB_GLOBAL, export=False, wrong_section=False, branch_displacement=4
):
    text_address = 0x02000000
    target_address = text_address + 4
    sections = [
        ("", 0, 0, 0, b"", 0, 0, 0, 0),
        (
            ".text",
            1,
            0x6,
            text_address,
            struct.pack(">II", 0x48000000 | branch_displacement, 0x60000000),
            0,
            0,
            4,
            0,
        ),
        (
            ".rela.text",
            normalizer.SHT_RELA,
            0,
            0,
            struct.pack(">IIi", text_address, (1 << 8) | normalizer.R_PPC_LOCAL24PC, 4),
            3,
            1,
            4,
            12,
        ),
        (".symtab", normalizer.SHT_SYMTAB, 0, 0, None, 4, 0, 4, 16),
        (".strtab", 3, 0, 0, b"\0test_target\0", 0, 0, 1, 0),
    ]
    if export:
        sections.extend(
            [
                (
                    ".fexports",
                    0x80000001,
                    0,
                    0xC0000020,
                    struct.pack(">II II", 1, 0xA7FEF0C2, target_address, 16) + b"test\0",
                    0,
                    0,
                    4,
                    0,
                ),
                (
                    ".rela.fexports",
                    normalizer.SHT_RELA,
                    0,
                    0,
                    struct.pack(
                        ">IIi",
                        0xC0000028,
                        (1 << 8) | normalizer.R_PPC_ADDR32,
                        4,
                    ),
                    3,
                    5,
                    4,
                    12,
                ),
            ]
        )
    symbol_section = 1
    if wrong_section:
        symbol_section = len(sections)
        sections.append((".data", 1, 0x3, 0x02001000, b"data", 0, 0, 4, 0))
    symbol_names = b"\0test_target\0"
    symbol = struct.pack(
        ">IIIBBH",
        1,
        text_address,
        4,
        (binding << 4) | normalizer.STT_FUNC,
        visibility,
        symbol_section,
    )
    sections[3] = (
        ".symtab",
        normalizer.SHT_SYMTAB,
        0,
        0,
        b"\0" * 16 + symbol,
        4,
        0,
        4,
        16,
    )
    shstrtab = bytearray(b"\0")
    name_offsets = []
    for section in sections:
        name_offsets.append(len(shstrtab))
        shstrtab.extend(section[0].encode("ascii") + b"\0")
    name_offsets.append(len(shstrtab))
    shstrtab.extend(b".shstrtab\0")
    sections.append((".shstrtab", 3, 0, 0, bytes(shstrtab), 0, 0, 1, 0))

    output = bytearray(b"\0" * 52)
    section_headers = [(0, 0, 0, 0, 0, 0, 0, 0, 0, 0)]
    for index, section in enumerate(sections[1:], start=1):
        _, kind, flags, address, data, link, info, alignment, entry_size = section
        alignment = max(alignment, 1)
        output.extend(b"\0" * (-len(output) % alignment))
        offset = len(output)
        output.extend(data)
        section_headers.append(
            (
                name_offsets[index],
                kind,
                flags,
                address,
                offset,
                len(data),
                link,
                info,
                alignment,
                entry_size,
            )
        )
    output.extend(b"\0" * (-len(output) % 4))
    section_header_offset = len(output)
    for header in section_headers:
        output.extend(struct.pack(">IIIIIIIIII", *header))

    output[:16] = b"\x7fELF\x01\x02\x01" + b"\0" * 9
    struct.pack_into(
        ">HHIIIIIHHHHHH",
        output,
        16,
        2,
        20,
        1,
        text_address,
        0,
        section_header_offset,
        0,
        52,
        0,
        0,
        40,
        len(section_headers),
        len(section_headers) - 1,
    )
    return bytes(output)


class Local24PcNormalizationTests(unittest.TestCase):
    def normalize_fixture(self, data):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        path = Path(temporary.name) / "fixture.elf"
        path.write_bytes(data)
        return path

    def test_normalizes_hidden_function_branch_without_changing_other_fields(self):
        original = synthetic_local24pc_elf(visibility=normalizer.STV_HIDDEN)
        path = self.normalize_fixture(original)
        pltrel24, local24pc = normalizer.normalize(path)

        self.assertEqual((pltrel24, local24pc), (0, 1))
        updated = path.read_bytes()
        old_sections, _ = normalizer.parse_sections(original)
        new_sections, _ = normalizer.parse_sections(updated)
        old_rela = next(s for s in old_sections if s.name == ".rela.text")
        new_rela = next(s for s in new_sections if s.name == ".rela.text")
        self.assertEqual(
            struct.unpack_from(">IIi", updated, new_rela.offset),
            (
                struct.unpack_from(">I", original, old_rela.offset)[0],
                (1 << 8) | normalizer.R_PPC_REL24,
                struct.unpack_from(">i", original, old_rela.offset + 8)[0],
            ),
        )
        old_text = next(s for s in old_sections if s.name == ".text")
        new_text = next(s for s in new_sections if s.name == ".text")
        self.assertEqual(
            updated[new_text.offset : new_text.offset + new_text.size],
            original[old_text.offset : old_text.offset + old_text.size],
        )

    def test_normalizes_hidden_weak_function_branch(self):
        path = self.normalize_fixture(
            synthetic_local24pc_elf(
                visibility=normalizer.STV_HIDDEN,
                binding=normalizer.STB_WEAK,
            )
        )
        self.assertEqual(normalizer.normalize(path), (0, 1))
        updated = path.read_bytes()
        sections, _ = normalizer.parse_sections(updated)
        rela = next(section for section in sections if section.name == ".rela.text")
        _, info, addend = struct.unpack_from(">IIi", updated, rela.offset)
        self.assertEqual(info, (1 << 8) | normalizer.R_PPC_REL24)
        self.assertEqual(addend, 4)

    def assert_rejected_without_mutation(self, data, message):
        path = self.normalize_fixture(data)
        before = path.read_bytes()
        with self.assertRaisesRegex(normalizer.InvalidElf, message):
            normalizer.normalize(path)
        self.assertEqual(path.read_bytes(), before)

    def test_rejects_default_visible_function_without_mutation(self):
        self.assert_rejected_without_mutation(
            synthetic_local24pc_elf(visibility=normalizer.STV_DEFAULT),
            "R_PPC_LOCAL24PC",
        )

    def test_rejects_exported_hidden_function_without_mutation(self):
        self.assert_rejected_without_mutation(
            synthetic_local24pc_elf(
                visibility=normalizer.STV_HIDDEN,
                export=True,
            ),
            "R_PPC_LOCAL24PC",
        )

    def test_rejects_hidden_function_in_wrong_section_without_mutation(self):
        self.assert_rejected_without_mutation(
            synthetic_local24pc_elf(
                visibility=normalizer.STV_HIDDEN,
                wrong_section=True,
            ),
            "R_PPC_LOCAL24PC",
        )

    def test_rejects_mismatched_branch_displacement_without_mutation(self):
        self.assert_rejected_without_mutation(
            synthetic_local24pc_elf(
                visibility=normalizer.STV_HIDDEN,
                branch_displacement=8,
            ),
            "linked branch instruction disagrees",
        )


if __name__ == "__main__":
    unittest.main()
