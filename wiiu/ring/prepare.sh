#!/usr/bin/env bash
set -euo pipefail

# Prepare exactly the locked crates.io release in a new disposable directory.
if [[ $# != 2 || "$2" != /* ]]; then
   printf 'Usage: %s /path/to/ring-0.17.14.crate /new/absolute/directory\n' "$0" >&2
   exit 2
fi
archive="$1"
destination="$2"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
expected=a4689e6c2294d81e88dc6261c768b63bc4fcdb852be6d1352498b114f61383b7
private_dir="$(mktemp -d "${TMPDIR:-/tmp}/horizonwiiu-ring.XXXXXXXX")"
created=0
cleanup() {
   status=$?
   rm -f -- "$private_dir/ring.crate"
   rmdir -- "$private_dir" || true
   if (( status != 0 && created )); then
      printf 'Incomplete destination preserved for inspection: %s\n' "$destination" >&2
   fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
# Hash and extract the same private copy, even if the original is replaced.
cp -- "$archive" "$private_dir/ring.crate"
actual="$(sha256sum -- "$private_dir/ring.crate" | cut -d ' ' -f 1)"
if [[ "$actual" != "$expected" ]]; then
   printf 'Ring archive checksum mismatch; no files created.\n' >&2
   exit 1
fi
# mkdir must create the directory itself: never merge with or overwrite a tree.
if ! mkdir -- "$destination"; then
   printf 'Destination must be a new directory with an existing parent.\n' >&2
   exit 1
fi
created=1
tar -xzf "$private_dir/ring.crate" --strip-components=1 --no-same-owner --no-same-permissions -C "$destination"
patch --batch --fuzz=0 --forward -p1 -d "$destination" < "$script_dir/ring-0.17.14-cafe.patch"
printf 'Prepared Ring at %s\n' "$destination"
