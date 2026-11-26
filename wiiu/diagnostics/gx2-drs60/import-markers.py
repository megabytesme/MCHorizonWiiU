#!/usr/bin/env python3
"""Check that internal RPX import markers start their import sections."""

import argparse
import re
import subprocess

IMPORT_SECTION = re.compile(
    r"\s*\[\s*(\d+)\]\s+(\.\w+import_\w+)\s+SHT_RPL_IMPORTS\s+([\da-f]{8})\b", re.I
)
IMPORT_MARKER = re.compile(
    r"\s*\d+\s+([\da-f]{8})\s+\d+\s+NOTYPE\s+\w+\s+(\d+)\s+(__\w+import_\w+)$", re.I
)


def read_rpl(option, path):
    return subprocess.check_output(["readrpl", option, path], text=True).splitlines()


def count_markers(path):
    sections = {}
    for line in read_rpl("-S", path):
        if match := IMPORT_SECTION.match(line):
            sections[int(match[1])] = match[2], int(match[3], 16)

    count = 0
    displaced = []
    for line in read_rpl("-s", path):
        if match := IMPORT_MARKER.match(line):
            count += 1
            section = sections.get(int(match[2]))
            if section is None:
                displaced.append(f"{match[3]} has no import section")
                continue
            name, base = section
            offset = int(match[1], 16) - base
            if offset:
                displaced.append(f"{match[3]} is at {offset:+#x} in {name}")
    return count, displaced


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rpx")
    args = parser.parse_args()
    try:
        count, displaced = count_markers(args.rpx)
        if not count:
            raise ValueError("no import markers found in the probe")
        if displaced:
            raise ValueError("; ".join(displaced))
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{args.rpx}: {error}\n")
    print(f"{args.rpx}: verified {count} import marker addresses")


if __name__ == "__main__":
    main()
