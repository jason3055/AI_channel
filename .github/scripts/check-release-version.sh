#!/usr/bin/env bash
set -euo pipefail

tag="${1:-}"
if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "release tag must look like vX.Y.Z, got: ${tag:-<empty>}" >&2
  exit 1
fi

version="${tag#v}"

check_manifest_version() {
  local manifest="$1"
  local actual
  actual="$(sed -n 's/^version = "\(.*\)"/\1/p' "$manifest" | head -n 1)"
  if [[ "$actual" != "$version" ]]; then
    echo "$manifest version is $actual, expected $version from $tag" >&2
    exit 1
  fi
}

check_lock_version() {
  local package="$1"
  local actual
  actual="$(
    awk -v package="$package" '
      /^\[\[package\]\]/ { in_pkg=1; name=""; version="" }
      in_pkg && /^name = / {
        name=$0
        sub(/^name = "/, "", name)
        sub(/"$/, "", name)
      }
      in_pkg && /^version = / {
        version=$0
        sub(/^version = "/, "", version)
        sub(/"$/, "", version)
        if (name == package) {
          print version
          exit
        }
      }
    ' Cargo.lock
  )"
  if [[ "$actual" != "$version" ]]; then
    echo "Cargo.lock package $package version is $actual, expected $version from $tag" >&2
    exit 1
  fi
}

check_manifest_version crates/aichan/Cargo.toml
check_manifest_version crates/aichan-core/Cargo.toml
check_manifest_version crates/aichan-server/Cargo.toml

check_lock_version aichan
check_lock_version aichan-core
check_lock_version aichan-server

echo "release version check passed for $tag"
