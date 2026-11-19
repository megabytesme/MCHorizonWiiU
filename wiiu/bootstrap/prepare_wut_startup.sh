#!/usr/bin/env bash
# Select WUT's RPX entry without altering the Rust compiler's static library.
set -euo pipefail

if [[ $# -ne 4 ]]; then
   echo "usage: $0 <rust-archive> <wut-archive> <output-archive> <output-crt0>" >&2
   exit 2
fi
input=$1
wut=$2
output=$3
crt0=$4
ar=${POWERPC_AR:-powerpc-eabi-ar}
nm=${POWERPC_NM:-powerpc-eabi-nm}
objcopy=${POWERPC_OBJCOPY:-powerpc-eabi-objcopy}
work=$(mktemp -d "${TMPDIR:-/tmp}/horizonwiiu-startup.XXXXXX")
trap 'rm -rf -- "$work"' EXIT

# The Cafe std archive supplies its own __rpx_start in the same object as
# other std code. Localize only this symbol in a copy of that member so WUT's
# startup can supply __preinit_user and initialize its heaps and threads.
mapfile -t members < <("$nm" -A --defined-only "$input" 2>/dev/null | awk '
   $NF == "__rpx_start" && $(NF-1) ~ /^[A-Z]$/ {
      line = $0
      sub(/[[:space:]]+[A-Z][[:space:]]+__rpx_start$/, "", line)
      sub(/:[^:]*$/, "", line)
      sub(/^.*:/, "", line)
      print line
   }
')
if [[ ${#members[@]} -ne 1 ]]; then
   echo "expected exactly one global Rust __rpx_start in $input; found ${#members[@]}" >&2
   exit 1
fi
member=${members[0]}
if [[ $("$ar" t "$input" | awk -v name="$member" '$0 == name { n++ } END { print n+0 }') -ne 1 ]]; then
   echo "archive member $member is not unique" >&2
   exit 1
fi
"$ar" p "$input" "$member" > "$work/$member"
"$objcopy" --localize-symbol=__rpx_start "$work/$member"
if "$nm" -g --defined-only "$work/$member" | awk '$NF == "__rpx_start" { found=1 } END { exit !found }'; then
   echo "could not localize Rust __rpx_start in $member" >&2
   exit 1
fi
cp -- "$input" "$work/worldgen.a"
"$ar" r "$work/worldgen.a" "$work/$member"
"$ar" p "$wut" crt0_rpx.o > "$work/crt0_rpx.o"
if [[ ! -s "$work/crt0_rpx.o" ]]; then
   echo "WUT archive did not provide crt0_rpx.o" >&2
   exit 1
fi
mv -f -- "$work/worldgen.a" "$output"
mv -f -- "$work/crt0_rpx.o" "$crt0"
