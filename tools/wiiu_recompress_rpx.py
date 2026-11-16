#!/usr/bin/env python3
"""Recompress compressed RPX RELA sections at zlib level 9.

This is a bounded packaging experiment for the observed Cafe ELF32/RPX
layout. It preserves relocation bytes, imports, exports, all section offsets,
the file length, CRC values, and FILEINFO. The source is never overwritten;
the destination must not already exist.

The script validates the ELF/RPX header, section and file bounds, section
non-overlap, every compressed section stream and raw-size prefix, and the
per-section CRC table before and after rewriting. It deliberately does not
interpret relocation records or alter source code.
"""

from __future__ import annotations

import argparse
import binascii
import os
import struct
import tempfile
import zlib
from dataclasses import dataclass
from pathlib import Path


ELF_HEADER_SIZE = 52
ELF32_SECTION_HEADER_SIZE = 40
ELF32_RELA_SIZE = 12
ELF_TYPE_RPX = 0xFE01
ELF_MACHINE_PPC = 20
ELF_VERSION_CURRENT = 1
ELF_OSABI_CAFE = 0xCA
ELF_ABIVERSION_CAFE = 0xFE
SHT_NULL = 0
SHT_STRTAB = 3
SHT_RELA = 4
SHT_NOBITS = 8
SHT_RPL_CRCS = 0x80000003
SHT_RPL_FILEINFO = 0x80000004
SHF_RPL_ZLIB_COMPRESSED = 0x08000000
RPL_FILEINFO_MAGIC = 0xCAFE0402
MAX_FILE_SIZE = 64 * 1024 * 1024
MAX_SECTION_UNCOMPRESSED_SIZE = 64 * 1024 * 1024
MAX_TOTAL_UNCOMPRESSED_SIZE = 128 * 1024 * 1024


class RpxError(ValueError):
    """The input is not a safe, supported RPX for this experiment."""


@dataclass(frozen=True)
class Section:
    index: int
    name: str
    kind: int
    flags: int
    address: int
    offset: int
    size: int
    link: int
    info: int
    alignment: int
    entry_size: int
    header_offset: int

    @property
    def compressed(self) -> bool:
        return bool(self.flags & SHF_RPL_ZLIB_COMPRESSED)


@dataclass(frozen=True)
class RpxImage:
    data: bytes
    section_table_offset: int
    section_count: int
    section_name_index: int
    sections: tuple[Section, ...]
    crc_section_index: int
    fileinfo_section_index: int


@dataclass(frozen=True)
class RecompressResult:
    data: bytes
    sections_recompressed: int
    bytes_saved: int


def _u16(data: bytes | bytearray, offset: int) -> int:
    return struct.unpack_from(">H", data, offset)[0]


def _u32(data: bytes | bytearray, offset: int) -> int:
    return struct.unpack_from(">I", data, offset)[0]


def _decode_compressed_section(stored: bytes, section_name: str) -> bytes:
    if len(stored) < 6:
        raise RpxError(f"{section_name}: compressed section is shorter than its prefix and zlib header")
    expected_size = int.from_bytes(stored[:4], "big")
    if expected_size > MAX_SECTION_UNCOMPRESSED_SIZE:
        raise RpxError(f"{section_name}: uncompressed size exceeds the safety limit")
    decoder = zlib.decompressobj()
    try:
        raw = decoder.decompress(stored[4:], expected_size + 1)
        if decoder.unconsumed_tail:
            raise RpxError(f"{section_name}: compressed stream exceeds its declared size")
        raw += decoder.flush()
    except zlib.error as error:
        raise RpxError(f"{section_name}: invalid zlib stream: {error}") from error
    if not decoder.eof:
        raise RpxError(f"{section_name}: truncated zlib stream")
    if decoder.unused_data:
        raise RpxError(f"{section_name}: trailing bytes after zlib stream")
    if len(raw) != expected_size:
        raise RpxError(
            f"{section_name}: size prefix says {expected_size}, decompressed to {len(raw)} bytes"
        )
    return raw


def _stored_section(data: bytes | bytearray, section: Section) -> bytes:
    if section.kind == SHT_NOBITS or section.size == 0:
        return b""
    return bytes(data[section.offset : section.offset + section.size])


def _raw_section(data: bytes | bytearray, section: Section) -> bytes:
    stored = _stored_section(data, section)
    if section.compressed:
        return _decode_compressed_section(stored, section.name or f"section[{section.index}]")
    return stored


def _parse_rpx(data: bytes) -> RpxImage:
    if len(data) < ELF_HEADER_SIZE:
        raise RpxError("input is shorter than an ELF32 header")
    if len(data) > MAX_FILE_SIZE:
        raise RpxError("input exceeds the RPX file-size safety limit")
    if data[:4] != b"\x7fELF":
        raise RpxError("input has no ELF magic")
    if data[4] != 1 or data[5] != 2 or data[6] != ELF_VERSION_CURRENT:
        raise RpxError("expected a big-endian ELF32 image")
    if data[7] != ELF_OSABI_CAFE or data[8] != ELF_ABIVERSION_CAFE:
        raise RpxError("expected the Cafe OS ABI marker")

    if _u16(data, 16) != ELF_TYPE_RPX or _u16(data, 18) != ELF_MACHINE_PPC:
        raise RpxError("expected a 32-bit PowerPC RPX (ET_RPX)")
    if _u32(data, 20) != ELF_VERSION_CURRENT:
        raise RpxError("unsupported ELF header version")
    program_header_offset = _u32(data, 28)
    section_table_offset = _u32(data, 32)
    header_size = _u16(data, 40)
    program_header_entry_size = _u16(data, 42)
    program_header_count = _u16(data, 44)
    section_header_entry_size = _u16(data, 46)
    section_count = _u16(data, 48)
    section_name_index = _u16(data, 50)

    if header_size != ELF_HEADER_SIZE:
        raise RpxError("unexpected ELF32 header size")
    if program_header_offset != 0 or program_header_entry_size != 0 or program_header_count != 0:
        raise RpxError("RPX must not contain program headers")
    if section_header_entry_size != ELF32_SECTION_HEADER_SIZE or section_count == 0:
        raise RpxError("unsupported section-header table layout")
    if section_name_index >= section_count:
        raise RpxError("section-name string-table index is out of range")
    section_table_size = section_count * section_header_entry_size
    section_table_end = section_table_offset + section_table_size
    if section_table_offset < header_size or section_table_end > len(data):
        raise RpxError("section-header table is outside the file")

    raw_sections: list[tuple[int, ...]] = []
    for index in range(section_count):
        header_offset = section_table_offset + index * section_header_entry_size
        fields = struct.unpack_from(">IIIIIIIIII", data, header_offset)
        raw_sections.append(fields)
        name_offset, kind, flags, _address, offset, size, _link, _info, _align, _entry_size = fields
        if index == 0 and kind != SHT_NULL:
            raise RpxError("section zero is not SHT_NULL")
        if kind == SHT_NOBITS or size == 0:
            continue
        end = offset + size
        if end > len(data):
            raise RpxError(f"section[{index}] file extent is outside the input")
        if offset < header_size or not (end <= section_table_offset or offset >= section_table_end):
            raise RpxError(f"section[{index}] overlaps an ELF header table")
        if flags & SHF_RPL_ZLIB_COMPRESSED and size < 6:
            raise RpxError(f"section[{index}] has a truncated compressed payload")

    name_table_fields = raw_sections[section_name_index]
    if name_table_fields[1] != SHT_STRTAB:
        raise RpxError("section-name table is not SHT_STRTAB")
    name_table_section = Section(
        index=section_name_index,
        name=".shstrtab",
        kind=name_table_fields[1],
        flags=name_table_fields[2],
        address=name_table_fields[3],
        offset=name_table_fields[4],
        size=name_table_fields[5],
        link=name_table_fields[6],
        info=name_table_fields[7],
        alignment=name_table_fields[8],
        entry_size=name_table_fields[9],
        header_offset=section_table_offset + section_name_index * section_header_entry_size,
    )
    names = _raw_section(data, name_table_section)
    if not names or names[0] != 0:
        raise RpxError("section-name table does not begin with NUL")

    sections: list[Section] = []
    for index, fields in enumerate(raw_sections):
        name_offset, kind, flags, address, offset, size, link, info, alignment, entry_size = fields
        if name_offset >= len(names):
            raise RpxError(f"section[{index}] name offset is out of range")
        name_end = names.find(b"\0", name_offset)
        if name_end < 0:
            raise RpxError(f"section[{index}] name is not NUL-terminated")
        try:
            name = names[name_offset:name_end].decode("ascii")
        except UnicodeDecodeError as error:
            raise RpxError(f"section[{index}] name is not ASCII") from error
        sections.append(
            Section(
                index=index,
                name=name,
                kind=kind,
                flags=flags,
                address=address,
                offset=offset,
                size=size,
                link=link,
                info=info,
                alignment=alignment,
                entry_size=entry_size,
                header_offset=section_table_offset + index * section_header_entry_size,
            )
        )

    file_sections = sorted(
        (section for section in sections if section.kind != SHT_NOBITS and section.size > 0),
        key=lambda section: (section.offset, section.offset + section.size),
    )
    previous: Section | None = None
    for section in file_sections:
        if previous is not None and section.offset < previous.offset + previous.size:
            raise RpxError(
                f"file sections overlap: section[{previous.index}] and section[{section.index}]"
            )
        previous = section

    crc_indices = [section.index for section in sections if section.kind == SHT_RPL_CRCS]
    fileinfo_indices = [section.index for section in sections if section.kind == SHT_RPL_FILEINFO]
    if len(crc_indices) != 1 or len(fileinfo_indices) != 1:
        raise RpxError("expected exactly one RPL CRC section and one FILEINFO section")
    crc_section_index = crc_indices[0]
    fileinfo_section_index = fileinfo_indices[0]
    crc_section = sections[crc_section_index]
    fileinfo_section = sections[fileinfo_section_index]
    if crc_section.compressed or fileinfo_section.compressed:
        raise RpxError("RPL CRC and FILEINFO sections must be uncompressed")
    if crc_section.size != section_count * 4:
        raise RpxError("RPL CRC table size does not match the section count")
    fileinfo = _raw_section(data, fileinfo_section)
    if len(fileinfo) < 4 or int.from_bytes(fileinfo[:4], "big") != RPL_FILEINFO_MAGIC:
        raise RpxError("RPL FILEINFO magic is invalid")

    crc_table = _raw_section(data, crc_section)
    total_uncompressed = 0
    raw_sections_cache: dict[int, bytes] = {}
    for section in sections:
        raw = _raw_section(data, section)
        total_uncompressed += len(raw)
        if total_uncompressed > MAX_TOTAL_UNCOMPRESSED_SIZE:
            raise RpxError("total uncompressed section data exceeds the safety limit")
        raw_sections_cache[section.index] = raw
    for section in sections:
        if section.index == crc_section_index:
            continue
        expected_crc = _u32(crc_table, section.index * 4)
        actual_crc = binascii.crc32(raw_sections_cache[section.index]) & 0xFFFFFFFF
        if expected_crc != actual_crc:
            raise RpxError(
                f"section[{section.index}] CRC mismatch: expected 0x{expected_crc:08x}, "
                f"computed 0x{actual_crc:08x}"
            )
    for section in sections:
        if section.kind == SHT_RELA:
            raw = raw_sections_cache[section.index]
            if section.entry_size != ELF32_RELA_SIZE or len(raw) % ELF32_RELA_SIZE:
                raise RpxError(f"{section.name or f'section[{section.index}]'} has invalid RELA layout")

    return RpxImage(
        data=data,
        section_table_offset=section_table_offset,
        section_count=section_count,
        section_name_index=section_name_index,
        sections=tuple(sections),
        crc_section_index=crc_section_index,
        fileinfo_section_index=fileinfo_section_index,
    )


def recompress_rpx(data: bytes) -> RecompressResult:
    """Return a verified copy with compressed RELA sections recompressed at 9."""
    image = _parse_rpx(data)
    patched = bytearray(data)
    section_updates: list[tuple[Section, bytes, bytes]] = []

    for section in image.sections:
        if section.kind != SHT_RELA or not section.compressed:
            continue
        raw = _raw_section(data, section)
        replacement = len(raw).to_bytes(4, "big") + zlib.compress(raw, 9)
        if len(replacement) < section.size:
            section_updates.append((section, raw, replacement))

    if not section_updates:
        raise RpxError("zlib level 9 produced no RELA section size savings")

    total_saved = 0
    expected_sizes = {section.index: section.size for section in image.sections}
    for section, raw, replacement in section_updates:
        old_size = section.size
        if len(replacement) >= old_size:
            raise RpxError(f"{section.name}: replacement does not fit the original extent")
        if _decode_compressed_section(replacement, section.name) != raw:
            raise RpxError(f"{section.name}: level-9 round-trip changed relocation bytes")
        patched[section.offset : section.offset + len(replacement)] = replacement
        tail_start = section.offset + len(replacement)
        tail_end = section.offset + old_size
        patched[tail_start:tail_end] = bytes(tail_end - tail_start)
        struct.pack_into(">I", patched, section.header_offset + 20, len(replacement))
        expected_sizes[section.index] = len(replacement)
        total_saved += old_size - len(replacement)

    candidate = bytes(patched)
    if len(candidate) != len(data):
        raise RpxError("candidate changed file length")
    verified = _parse_rpx(candidate)

    if verified.section_count != image.section_count:
        raise RpxError("candidate changed section count")
    if verified.sections != tuple(
        Section(
            index=section.index,
            name=section.name,
            kind=section.kind,
            flags=section.flags,
            address=section.address,
            offset=section.offset,
            size=expected_sizes[section.index],
            link=section.link,
            info=section.info,
            alignment=section.alignment,
            entry_size=section.entry_size,
            header_offset=section.header_offset,
        )
        for section in image.sections
    ):
        raise RpxError("candidate changed section metadata beyond RELA sizes")

    for old_section, new_section in zip(image.sections, verified.sections, strict=True):
        if _raw_section(data, old_section) != _raw_section(candidate, new_section):
            raise RpxError(f"section[{old_section.index}] uncompressed contents changed")
        if old_section.kind in (SHT_RPL_CRCS, SHT_RPL_FILEINFO):
            if _stored_section(data, old_section) != _stored_section(candidate, new_section):
                raise RpxError(f"{old_section.name}: metadata contents changed")
    for section, _raw, replacement in section_updates:
        if _stored_section(candidate, verified.sections[section.index]) != replacement:
            raise RpxError(f"{section.name}: candidate stream differs from verified replacement")

    return RecompressResult(candidate, len(section_updates), total_saved)


def recompress_file(input_path: Path, output_path: Path) -> RecompressResult:
    source = input_path.expanduser().resolve(strict=True)
    requested_destination = output_path.expanduser()
    if requested_destination.is_symlink():
        raise RpxError("output path already exists")
    destination = requested_destination.resolve(strict=False)
    if source == destination:
        raise RpxError("input and output paths must be distinct")
    if destination.exists() or destination.is_symlink():
        raise RpxError("output path already exists")
    if not source.is_file():
        raise RpxError("input path is not a regular file")
    if not destination.parent.is_dir():
        raise RpxError("output parent directory does not exist")

    try:
        with source.open("rb") as source_file:
            before = os.fstat(source_file.fileno())
            if before.st_size > MAX_FILE_SIZE:
                raise RpxError("input exceeds the RPX file-size safety limit")
            source_bytes = source_file.read(MAX_FILE_SIZE + 1)
            after = os.fstat(source_file.fileno())
        if len(source_bytes) > MAX_FILE_SIZE or after.st_size > MAX_FILE_SIZE:
            raise RpxError("input exceeds the RPX file-size safety limit")
        if (
            len(source_bytes) != before.st_size
            or after.st_size != before.st_size
            or after.st_mtime_ns != before.st_mtime_ns
        ):
            raise RpxError("input changed while it was being read")
        result = recompress_rpx(source_bytes)
    except OSError as error:
        raise RpxError(f"cannot read input: {error}") from error

    temp_path: Path | None = None
    try:
        descriptor, temporary = tempfile.mkstemp(
            prefix=f".{destination.name}.", suffix=".tmp", dir=destination.parent
        )
        temp_path = Path(temporary)
        with os.fdopen(descriptor, "wb") as temporary_file:
            temporary_file.write(result.data)
            temporary_file.flush()
            os.fsync(temporary_file.fileno())
        # A hard link publishes atomically and fails rather than replacing a
        # destination created by another process after the initial check.
        os.link(temp_path, destination)
        return result
    except OSError as error:
        raise RpxError(f"cannot publish output without overwriting: {error}") from error
    finally:
        if temp_path is not None:
            try:
                temp_path.unlink(missing_ok=True)
            except OSError:
                pass


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("input", type=Path, help="existing RPX to read")
    parser.add_argument("output", type=Path, help="new, distinct RPX path to create")
    args = parser.parse_args(argv)
    try:
        result = recompress_file(args.input, args.output)
    except (OSError, RpxError) as error:
        parser.error(str(error))
    print(
        f"Recompressed {result.sections_recompressed} RELA sections; "
        f"saved {result.bytes_saved:,} stored bytes."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
