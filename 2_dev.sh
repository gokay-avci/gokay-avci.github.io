#!/usr/bin/env bash
set -euo pipefail
unset NO_COLOR

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$repo_root"

if ! command -v zola >/dev/null 2>&1; then
  echo "❌ Zola is required but was not found in PATH." >&2
  exit 127
fi

BASE_URL="http://127.0.0.1:1111"
echo "🧪 Building demos for local development..."
"$repo_root/scripts/build_demos.sh" dev
echo "⚡ Starting Zola (Port 1111)..."
echo "🌐 Local base URL: $BASE_URL"
zola serve --port 1111 --base-url "$BASE_URL" --no-port-append --open --store-html
