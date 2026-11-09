#!/usr/bin/env python3
"""Normalize final-link PPC branch relocations unsupported by cargo-wiiu.

Only local, already-resolved branches inside the Cafe .text section qualify.
The checks are intentionally strict: an import, interposable symbol, other
section, or branch whose encoded displacement disagrees with ELF metadata is
left for the caller to diagnose rather than rewritten.
"""

from __future__ import annotations

import argparse
import os
import struct
import tempfile
from dataclasses import dataclass
from pathlib import Path


SHT_SYMTAB = 2
SHT_RELA = 4
SHT_DYNSYM = 11
ET_EXEC = 2
EM_PPC = 20
R_PPC_REL24 = 10
R_PPC_ADDR32 = 1
R_PPC_PLTREL24 = 18
R_PPC_LOCAL24PC = 23
STB_LOCAL = 0
STB_GLOBAL = 1
STB_WEAK = 2
STT_FUNC = 2
STT_SECTION = 3
STV_DEFAULT = 0
STV_HIDDEN = 2
STV_PROTECTED = 3


class InvalidElf(ValueError):
    pass


@dataclass(frozen=True)
class Section:
    name_offset: int
    kind: int
    address: int
    offset: int
    size: int
    link: int
    info: int
    entry_size: int
    name: str = ""


@dataclass(frozen=True)
class Symbol:
    name: str
    value: int
    kind: int
    binding: int
    visibility: int
    section_index: int


def u16(data: bytes | bytearray, offset: int) -> int:
    return struct.unpack_from(">H", data, offset)[0]


def u32(data: bytes | bytearray, offset: int) -> int:
    return struct.unpack_from(">I", data, offset)[0]


def s32(data: bytes | bytearray, offset: int) -> int:
    return struct.unpack_from(">i", data, offset)[0]


def checked_slice(data: bytes | bytearray, start: int, size: int, what: str) -> bytes:
    end = start + size
    if start < 0 or size < 0 or end > len(data):
        raise InvalidElf(f"{what} is outside the ELF file")
    return bytes(data[start:end])


def c_string(table: bytes, offset: int, what: str) -> str:
    if offset < 0 or offset >= len(table):
        raise InvalidElf(f"{what} name offset is out of range")
    end = table.find(b"\0", offset)
    if end < 0:
        raise InvalidElf(f"{what} name is not NUL-terminated")
    return table[offset:end].decode("utf-8", errors="replace")


def parse_sections(data: bytes) -> tuple[list[Section], int]:
    if len(data) < 52 or data[:4] != b"\x7fELF":
        raise InvalidElf("input is not an ELF file")
    if data[4] != 1 or data[5] != 2:
        raise InvalidElf("expected a big-endian ELF32 Cafe image")
    if u16(data, 16) != ET_EXEC or u16(data, 18) != EM_PPC:
        raise InvalidElf("expected a linked 32-bit PowerPC executable")

    section_offset = u32(data, 32)
    section_entry_size = u16(data, 46)
    section_count = u16(data, 48)
    name_table_index = u16(data, 50)
    if section_entry_size != 40 or section_count == 0 or name_table_index >= section_count:
        raise InvalidElf("unsupported or malformed ELF section table")
    checked_slice(data, section_offset, section_entry_size * section_count, "section table")

    raw: list[Section] = []
    for index in range(section_count):
        at = section_offset + index * section_entry_size
        raw.append(
            Section(
                name_offset=u32(data, at),
                kind=u32(data, at + 4),
                address=u32(data, at + 12),
                offset=u32(data, at + 16),
                size=u32(data, at + 20),
                link=u32(data, at + 24),
                info=u32(data, at + 28),
                entry_size=u32(data, at + 36),
            )
        )

    names_section = raw[name_table_index]
    names = checked_slice(data, names_section.offset, names_section.size, "section-name table")
    sections = [
        Section(
            name_offset=s.name_offset,
            kind=s.kind,
            address=s.address,
            offset=s.offset,
            size=s.size,
            link=s.link,
            info=s.info,
            entry_size=s.entry_size,
            name=c_string(names, s.name_offset, "section"),
        )
        for s in raw
    ]
    text_indices = [index for index, section in enumerate(sections) if section.name == ".text"]
    if len(text_indices) != 1:
        raise InvalidElf("expected exactly one .text section")
    return sections, text_indices[0]


def read_symbol(data: bytes, sections: list[Section], symtab_index: int, index: int) -> Symbol:
    if symtab_index >= len(sections):
        raise InvalidElf("relocation section references an invalid symbol table")
    symtab = sections[symtab_index]
    if symtab.kind not in (SHT_SYMTAB, SHT_DYNSYM) or symtab.entry_size != 16:
        raise InvalidElf("relocation section references an unsupported symbol table")
    if index >= symtab.size // symtab.entry_size:
        raise InvalidElf("relocation references an invalid symbol")
    if symtab.link >= len(sections):
        raise InvalidElf("symbol table references an invalid string table")

    strings_section = sections[symtab.link]
    strings = checked_slice(data, strings_section.offset, strings_section.size, "symbol strings")
    at = symtab.offset + index * symtab.entry_size
    name = c_string(strings, u32(data, at), "symbol")
    info = data[at + 12]
    return Symbol(
        name=name,
        value=u32(data, at + 4),
        kind=info & 0x0F,
        binding=info >> 4,
        visibility=data[at + 13] & 0x03,
        section_index=u16(data, at + 14),
    )


def signed_branch_displacement(instruction: int) -> int:
    displacement = instruction & 0x03FFFFFC
    if displacement & 0x02000000:
        displacement -= 0x04000000
    return displacement


def normalize(path: Path) -> tuple[int, int]:
    original = path.read_bytes()
    data = bytearray(original)
    sections, text_index = parse_sections(original)
    text = sections[text_index]
    checked_slice(original, text.offset, text.size, ".text")
    changed = {R_PPC_PLTREL24: 0, R_PPC_LOCAL24PC: 0}

    exported_addresses: set[int] = set()
    # Cafe modules export through .fexports; ELF default visibility alone does
    # not place a function in the loader's export table.
    fexports_indices = [index for index, section in enumerate(sections) if section.name == ".fexports"]
    if len(fexports_indices) > 1:
        raise InvalidElf("expected at most one .fexports section")
    if fexports_indices:
        fexports_index = fexports_indices[0]
        fexports = sections[fexports_index]
        checked_slice(original, fexports.offset, fexports.size, ".fexports")
        if fexports.size < 8 or u32(original, fexports.offset + 4) != 0xA7FEF0C2:
            raise InvalidElf(".fexports has an invalid Cafe export header")
        export_count = u32(original, fexports.offset)
        export_table_end = 8 + export_count * 8
        if export_table_end > fexports.size:
            raise InvalidElf(".fexports export records extend past the section")
        export_relocations = [
            section for section in sections
            if section.name == ".rela.fexports"
        ]
        if len(export_relocations) != 1:
            raise InvalidElf("cannot prove the linked Cafe export list")
        export_rela = export_relocations[0]
        if export_rela.kind != SHT_RELA or export_rela.info != fexports_index:
            raise InvalidElf(".rela.fexports does not target .fexports")
        if export_rela.entry_size != 12 or export_rela.size % export_rela.entry_size:
            raise InvalidElf(".rela.fexports has an invalid ELF32 RELA layout")
        if export_rela.size // export_rela.entry_size != export_count:
            raise InvalidElf(".fexports count does not match its symbol relocations")
        export_symbol_table = sections[export_rela.link] if export_rela.link < len(sections) else None
        if export_symbol_table is None or export_symbol_table.kind not in (SHT_SYMTAB, SHT_DYNSYM):
            raise InvalidElf(".rela.fexports references an invalid symbol table")
        seen_export_offsets: set[int] = set()
        seen_export_names: set[str] = set()
        for index in range(export_count):
            entry_offset = fexports.offset + 8 + index * 8
            address_field = fexports.address + 8 + index * 8
            name_offset = u32(original, entry_offset + 4)
            if name_offset < export_table_end or name_offset >= fexports.size:
                raise InvalidElf(".fexports name offset is outside its name table")
            export_name = c_string(
                checked_slice(original, fexports.offset, fexports.size, ".fexports"),
                name_offset,
                ".fexports entry",
            )
            if not export_name or export_name in seen_export_names:
                raise InvalidElf(".fexports contains an empty or duplicate export name")
            seen_export_names.add(export_name)

            at = export_rela.offset + index * export_rela.entry_size
            checked_slice(original, at, export_rela.entry_size, ".rela.fexports entry")
            relocation_offset = u32(original, at)
            info = u32(original, at + 4)
            relocation_type = info & 0xFF
            if relocation_offset != address_field or relocation_offset in seen_export_offsets:
                raise InvalidElf(".rela.fexports does not uniquely relocate each export address field")
            seen_export_offsets.add(relocation_offset)
            if relocation_type != R_PPC_ADDR32:
                raise InvalidElf(".rela.fexports export address is not an R_PPC_ADDR32 relocation")
            export_symbol = read_symbol(original, sections, export_rela.link, info >> 8)
            if export_symbol.section_index == 0:
                raise InvalidElf(f".fexports entry {export_name} refers to an undefined symbol")
            export_address = export_symbol.value + s32(original, at + 8)
            if not 0 <= export_address <= 0xFFFFFFFF:
                raise InvalidElf(f".fexports entry {export_name} resolves outside ELF32 address space")
            if u32(original, entry_offset) != export_address:
                raise InvalidElf(f".fexports entry {export_name} disagrees with its linked address")
            exported_addresses.add(export_address)

    for rela in sections:
        if rela.kind != SHT_RELA:
            continue
        if rela.entry_size != 12 or rela.size % rela.entry_size:
            raise InvalidElf(f"{rela.name} has an invalid ELF32 RELA layout")
        if rela.info >= len(sections):
            raise InvalidElf(f"{rela.name} references an invalid target section")
        target_section = sections[rela.info]
        if rela.link >= len(sections):
            raise InvalidElf(f"{rela.name} references an invalid symbol table")

        for relative in range(0, rela.size, rela.entry_size):
            at = rela.offset + relative
            checked_slice(original, at, rela.entry_size, f"{rela.name} entry")
            info = u32(original, at + 4)
            relocation_type = info & 0xFF
            if relocation_type not in changed:
                continue
            symbol_index = info >> 8
            symbol = read_symbol(original, sections, rela.link, symbol_index)
            place = u32(original, at)
            addend = s32(original, at + 8)

            if rela.info != text_index or target_section.name != ".text":
                raise InvalidElf(f"{rela.name} contains a Cafe branch outside .text")
            if place < text.address or place + 4 > text.address + text.size:
                raise InvalidElf(f"{rela.name} branch site is outside .text")

            if relocation_type == R_PPC_LOCAL24PC:
                if symbol.kind != STT_SECTION or symbol.section_index != text_index:
                    raise InvalidElf("R_PPC_LOCAL24PC does not target the local .text section")
            else:
                if symbol.kind != STT_FUNC or symbol.section_index != text_index:
                    raise InvalidElf("R_PPC_PLTREL24 is not bound to a function in .text")
                if symbol.binding not in (STB_LOCAL, STB_GLOBAL, STB_WEAK):
                    raise InvalidElf(f"R_PPC_PLTREL24 has unsupported binding for {symbol.name}")
                if symbol.binding != STB_LOCAL and symbol.visibility not in (
                    STV_DEFAULT,
                    STV_HIDDEN,
                    STV_PROTECTED,
                ):
                    raise InvalidElf(f"R_PPC_PLTREL24 target {symbol.name} is interposable")

            target = symbol.value + addend
            if relocation_type == R_PPC_PLTREL24 and target in exported_addresses:
                raise InvalidElf(f"R_PPC_PLTREL24 target {symbol.name} is exported by the Cafe module")
            if not text.address <= target < text.address + text.size:
                raise InvalidElf(f"{symbol.name or '.text'} branch target is outside .text")
            displacement = target - place
            if displacement % 4 or not -(1 << 25) <= displacement < (1 << 25):
                raise InvalidElf("branch displacement is unaligned or outside REL24 range")

            instruction_offset = text.offset + place - text.address
            instruction = u32(original, instruction_offset)
            if instruction >> 26 != 18 or instruction & 0x2:
                raise InvalidElf("branch relocation site is not a relative PPC branch")
            if signed_branch_displacement(instruction) != displacement:
                raise InvalidElf("linked branch instruction disagrees with its ELF target")

            data[at + 4 : at + 8] = struct.pack(">I", (info & 0xFFFFFF00) | R_PPC_REL24)
            changed[relocation_type] += 1

    if not any(changed.values()):
        raise InvalidElf("no unsupported Cafe branch relocations were found")

    with tempfile.NamedTemporaryFile(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent, delete=False) as out:
        temporary = Path(out.name)
        out.write(data)
        out.flush()
        os.fsync(out.fileno())
    try:
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)
    return changed[R_PPC_PLTREL24], changed[R_PPC_LOCAL24PC]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("elf", type=Path, help="linked Cafe ELF copied for RPX packaging")
    args = parser.parse_args()
    try:
        pltrel24, local24pc = normalize(args.elf)
    except (InvalidElf, OSError, struct.error) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")
    print(
        f"Normalized {pltrel24} local PLTREL24 and {local24pc} local LOCAL24PC "
        "branch relocations to REL24"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
