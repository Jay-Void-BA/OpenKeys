#!/bin/sh
set -eu
task_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
mkdir -p "$task_root/reference"
tab=$(printf '\t')
while IFS="$tab" read -r name url commit branch; do
    case "$name" in '#'*|'') continue ;; esac
    case "$name" in *[!a-z0-9-]*) exit 2 ;; esac
    task_repo="$task_root/reference/$name"
    if [ ! -d "$task_repo/.git" ]; then
        git init "$task_repo"
        git -C "$task_repo" remote add origin "$url"
        git -C "$task_repo" fetch --depth 1 origin "$commit"
        git -C "$task_repo" checkout --detach "$commit"
    fi
    actual=$(git -C "$task_repo" rev-parse HEAD)
    if [ "$actual" != "$commit" ]; then
        printf '%s\n' "Reference $name differs from lock; refusing to replace it." >&2
        exit 1
    fi
    if [ -n "$(git -C "$task_repo" status --porcelain)" ]; then
        printf '%s\n' "Reference $name has local edits; inspect before using it." >&2
        exit 1
    fi
    printf '%s\n' "$name $actual verified"
done < "$task_root/references.lock.tsv"
