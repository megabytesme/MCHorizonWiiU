#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
repo=$(realpath "${1:-../../../..}")
tmp=$(mktemp -d)
trap 'rm -f -- "$tmp/"{test_teardown,faults.inc,stage-observe.inc,retire.inc}; rmdir -- "$tmp"' EXIT
python3 -B - "$repo" "$tmp" <<'PY'
from pathlib import Path
import sys

repo, out = map(Path, sys.argv[1:])
diagnostics = repo / "wiiu/diagnostics"
source = (diagnostics / "gx2-independent-views/main.c").read_text()
stage = (diagnostics / "gx2-explicit-readback/stage.h").read_text()

def fragment(text, first, last):
    begin = text.index(first)
    return text[begin:text.index(last, begin)]

(out / "retire.inc").write_text(fragment(source, "static void retire_for_teardown", "static void quad"))
(out / "faults.inc").write_text(fragment(source, "   // Run the independent positive matrix", "   if (graphics) retire_for_teardown"))
(out / "stage-observe.inc").write_text(
    fragment(stage, "typedef struct {", "enum ReadStatus")
    + fragment(stage, "static void stage_observe", "static enum ReadStatus stage_read")
)
PY
"${CC:-cc}" -std=c17 -Wall -Wextra -Werror -pedantic -g -O1 -fsanitize=address,undefined \
  -fno-pie -no-pie -I "$tmp" -I "$repo/wiiu/diagnostics/gx2-independent-views" test_teardown.c -o "$tmp/test_teardown"
"$tmp/test_teardown"
