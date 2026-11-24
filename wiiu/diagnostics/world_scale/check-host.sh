#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
   echo "Usage: $0 ASSEMBLED_SOURCE_DIRECTORY" >&2
   exit 2
fi
source_dir=$(cd "$1" && pwd)
diagnostic_dir=$(cd "$(dirname "$0")" && pwd)
scratch=$(mktemp -d)
trap 'rm -rf -- "$scratch"' EXIT

cp "$diagnostic_dir/host_tests.rs" "$scratch/"
ln -s "$source_dir" "$scratch/source"
cat > "$scratch/report.c" <<'C'
void OSReport(const char *format, ...) { (void)format; }
C
"${CC:-cc}" -Wall -Wextra -Werror -c "$scratch/report.c" -o "$scratch/report.o"
"${RUSTC:-rustc}" --edition 2024 --test "$scratch/host_tests.rs" \
   -C link-arg="$scratch/report.o" -o "$scratch/host_tests"
"$scratch/host_tests"
