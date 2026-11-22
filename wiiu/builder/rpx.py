"""Measure an RPX against the observed Aroma system-title code heap."""
from struct import Struct


ELF_HEADER = Struct(">16sHHIIIIIHHHHHH")
SECTION_HEADER = Struct(">10I")
FILE_INFO = Struct(">24I")
CODE_BASE = 0x02000000
DATA_BASE = 0x10000000
SHT_PROGBITS = 1
SHT_NOBITS = 8
SHT_RPL_FILEINFO = 0x80000004
SHF_EXECINSTR = 4
SHF_DEFLATED = 0x08000000

# Aroma's Mocha wrapper reserves 64 MiB. The physical RC 15 log measured
# the contiguous space after heap bookkeeping and the allocator's extra
# 64 bytes for an already cache-line-aligned text allocation.
AROMA_CONTIGUOUS_BYTES = 67_096_512
CAFE_ALLOCATION_OVERHEAD = 64


def read_code_heap(path):
    """Check the decompressed code span and RPL file-info declaration."""
    with path.open("rb") as stream:
        size = stream.seek(0, 2)

        def read_at(offset, length):
            if offset + length > size:
                raise ValueError("truncated RPX")
            stream.seek(offset)
            return stream.read(length)

        (ident, kind, machine, version, entry, _, table, _, _, _, _,
         header_size, count, _) = ELF_HEADER.unpack(read_at(0, ELF_HEADER.size))
        if (ident[:9] != b"\x7fELF\x01\x02\x01\xca\xfe" or (kind, machine, version) != (0xFE01, 20, 1)
                or header_size != SECTION_HEADER.size or not count):
            raise ValueError("expected a big-endian PowerPC Cafe RPX")

        code = []
        file_info = []
        for index, section in enumerate(SECTION_HEADER.iter_unpack(read_at(table, count * header_size))):
            _, section_type, flags, address, offset, stored, _, _, alignment, _ = section
            if section_type == SHT_RPL_FILEINFO:
                if stored < FILE_INFO.size or flags & SHF_DEFLATED:
                    raise ValueError("unsupported Cafe RPL file info")
                file_info.append(FILE_INFO.unpack(read_at(offset, FILE_INFO.size)))
            if not CODE_BASE <= address < DATA_BASE:
                continue
            if section_type != SHT_NOBITS and offset + stored > size:
                raise ValueError("truncated RPX code section")
            expanded = stored
            if flags & SHF_DEFLATED:
                if stored <= 4 or section_type == SHT_NOBITS:
                    raise ValueError("invalid compressed RPX code section")
                expanded = int.from_bytes(read_at(offset, 4), "big")
            if address + expanded > DATA_BASE or not expanded:
                raise ValueError("RPX section extends outside the code region")
            code.append({"index": index, "address": address, "alignment": alignment,
                         "stored_bytes": stored, "expanded_bytes": expanded,
                         "executable": section_type == SHT_PROGBITS and bool(flags & SHF_EXECINSTR)})

    if len(file_info) != 1 or file_info[0][0] != 0xCAFE0402:
        raise ValueError("missing or unsupported Cafe RPL file info")
    if not any(item["executable"] and item["address"] <= entry < item["address"] + item["expanded_bytes"]
               for item in code):
        raise ValueError("RPX entry point lies outside executable code")
    _, declared, alignment, data_size, _, load_size, _, temp_size, tramp_adjust = file_info[0][:9]
    if not alignment or alignment & (alignment - 1) or tramp_adjust or file_info[0][18]:
        raise ValueError("unsupported Cafe text alignment or trampoline reservation")
    end = CODE_BASE
    for item in sorted(code, key=lambda item: item["address"]):
        if item["address"] < end:
            raise ValueError("overlapping RPX code sections")
        end = item["address"] + item["expanded_bytes"]
    span = end - CODE_BASE
    if ((span + alignment - 1) & -alignment) != declared:
        raise ValueError("RPX code span disagrees with the declared Cafe text size")

    # The largest code section alone caused the measured physical failure.
    # Budget the whole mapped span and round up to a conservative 64-byte
    # boundary to account for section gaps and any trailing alignment.
    needed = ((declared + 63) & -64) + CAFE_ALLOCATION_OVERHEAD
    return {
        "rpx_bytes": size,
        "code_sections": code,
        "expanded_executable_bytes": sum(item["expanded_bytes"] for item in code if item["executable"]),
        "code_span_bytes": span,
        "code_padding_bytes": declared - sum(item["expanded_bytes"] for item in code),
        "declared_text_bytes": declared,
        "declared_text_alignment": alignment,
        "declared_data_bytes": data_size,
        "declared_load_bytes": load_size,
        "declared_temp_bytes": temp_size,
        "estimated_text_allocation_bytes": needed,
        "observed_contiguous_bytes": AROMA_CONTIGUOUS_BYTES,
        "remaining_bytes": AROMA_CONTIGUOUS_BYTES - needed,
    }


def require_aroma_fit(report):
    if report["remaining_bytes"] < 0:
        raise ValueError(f'RPX exceeds the observed Aroma code heap by {-report["remaining_bytes"]:,} bytes')
