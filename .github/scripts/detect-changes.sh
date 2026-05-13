#!/usr/bin/env bash
set -euo pipefail

base="${1:-}"
head="${2:-HEAD}"
event_name="${GITHUB_EVENT_NAME:-push}"
output_file="${GITHUB_OUTPUT:-/dev/stdout}"

verify=false
deploy=false

mark_file() {
  local file="$1"

  case "$file" in
    Cargo.toml|Cargo.lock|Dockerfile|.dockerignore|.github/workflows/deploy.yml|.github/scripts/*)
      verify=true
      deploy=true
      ;;
    crates/aichan-core/*|crates/aichan-server/*)
      verify=true
      deploy=true
      ;;
    crates/aichan/*)
      verify=true
      ;;
    firestore.indexes.json)
      verify=false
      deploy=false
      ;;
    *)
      ;;
  esac
}

if [[ "$event_name" != "push" ]]; then
  verify=true
  deploy=true
elif [[ -n "${AICHAN_CHANGED_FILES:-}" ]]; then
  while IFS= read -r file; do
    [[ -n "$file" ]] && mark_file "$file"
  done <<< "$AICHAN_CHANGED_FILES"
elif [[ -z "$base" || "$base" =~ ^0+$ ]]; then
  verify=true
  deploy=true
else
  while IFS= read -r file; do
    [[ -n "$file" ]] && mark_file "$file"
  done < <(git diff --name-only "$base" "$head")
fi

{
  echo "verify=$verify"
  echo "deploy=$deploy"
} >> "$output_file"

echo "verify=$verify deploy=$deploy"
