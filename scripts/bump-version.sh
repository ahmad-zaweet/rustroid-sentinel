#!/usr/bin/env bash
# Bumps the crate version via `cargo-verctl` (Cargo.toml) and mirrors it into
# package.json/package-lock.json via `npm version`, so the Rust crate and the
# frontend asset build pipeline never drift apart. Also refreshes Cargo.lock's
# own-package entry so the bump is fully reflected in one commit.
#
# Usage:
#   scripts/bump-version.sh major|minor|patch
#   scripts/bump-version.sh --set 2.3.0
set -euo pipefail

cd "$(dirname "$0")/.."

usage() {
    echo "Usage: $0 <major|minor|patch|--set VERSION>" >&2
    exit 1
}

[[ $# -ge 1 ]] || usage

if [[ "$1" == "--set" ]]; then
    [[ $# -eq 2 ]] || usage
    cargo verctl --set "$2"
else
    case "$1" in
        major | minor | patch) ;;
        *) usage ;;
    esac
    cargo verctl --bump "$1"
fi

new_version=$(grep -m1 '^version' Cargo.toml | sed -E 's/version = "(.*)"/\1/')
echo "Cargo.toml   -> $new_version"

npm version "$new_version" --no-git-tag-version --allow-same-version >/dev/null
echo "package.json -> $new_version"

# Cargo.lock embeds this crate's own version; refresh it without touching
# dependency versions.
cargo check --quiet
echo "Cargo.lock   -> refreshed"

echo
echo "Bumped to $new_version. Review the diff, then commit."
