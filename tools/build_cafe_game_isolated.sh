#!/usr/bin/env bash
# Give independent build directories the same compiler-visible path.
set -euo pipefail

target=build-game
if [[ "${1:-}" == --hosted ]]; then
    target=build-hosted
    shift
fi

if (( $# < 2 )); then
    echo "Usage: $0 [--hosted] <build-directory> --assembly <path> --inputs <path> [build-game options]" >&2
    exit 2
fi

destination=$(realpath -m -- "$1")
shift
workspace=/var/tmp/horizon-wiiu-build
for argument in "$@"; do
    case "$argument" in
        --out*) echo "Pass the build directory as the first argument." >&2; exit 2 ;;
    esac
done

if [[ "$destination" == "$workspace" || "$destination" == "$workspace/"* ]]; then
    echo "The destination must be outside the compiler's mount point." >&2
    exit 2
fi

mkdir -p -- "$workspace" "$destination"
if [[ "$(realpath -- "$workspace")" != "$workspace" || -n "$(ls -A -- "$workspace")" ]]; then
    echo "The compiler mount point must be an empty directory: $workspace" >&2
    exit 2
fi

project=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
record=$(
    unshare --user --map-root-user --mount bash -c '
        set -euo pipefail
        mount --bind -- "$1" "$2"
        workspace=$2
        builder=$3
        target=$4
        shift 4
        python3 -B "$builder" "$target" --output "$workspace" "$@" |
            awk "{ if (NR > 1) print previous > \"/dev/stderr\"; previous=\$0 } END { print previous }"
    ' -- "$destination" "$workspace" "$project/wiiu/builder/build.py" "$target" "$@"
)
if [[ "$record" != "$workspace/"*/build-record.json ]]; then
    echo "The builder did not emit a verified record." >&2
    exit 1
fi
printf '%s\n' "$destination/${record#"$workspace/"}"
