#!/usr/bin/env bash
set -euo pipefail
unset NO_COLOR

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$repo_root"

if ! command -v zola >/dev/null 2>&1; then
  echo "❌ Zola is required but was not found in PATH." >&2
  exit 127
fi

echo "🏗️  Building Production..."
"$repo_root/scripts/build_demos.sh" release
echo "⚡ Generating Static Site..."
zola build
echo "✅ Build Complete in /public"
