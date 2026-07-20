#!/usr/bin/env bash
set -euo pipefail
unset NO_COLOR

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
profile=${1:-release}

case "$profile" in
  release)
    ;;
  dev)
    ;;
  *)
    echo "Usage: $0 [release|dev]" >&2
    exit 2
    ;;
esac

demo_output="$repo_root/static/demos"

if ! command -v cargo >/dev/null 2>&1; then
  echo "❌ Cargo is required but was not found in PATH." >&2
  exit 127
fi
if ! command -v trunk >/dev/null 2>&1; then
  echo "❌ Trunk is required but was not found in PATH." >&2
  exit 127
fi

rm -rf "$demo_output"
mkdir -p "$demo_output"

echo "🦀 Compiling Rust demos ($profile)..."
for demo_dir in "$repo_root"/demos/*; do
  [ -d "$demo_dir" ] || continue

  demo_name=${demo_dir##*/}
  if [ -x "$demo_dir/scripts/build-static.sh" ]; then
    echo "   • $demo_name (static WASM)"
    EVOKUPS_DIST_DIR="$demo_output/$demo_name" \
      EVOKUPS_PROFILE="$profile" \
      "$demo_dir/scripts/build-static.sh"
  elif [ -f "$demo_dir/Trunk.toml" ]; then
    echo "   • $demo_name (Trunk)"
    if [ "$profile" = "release" ]; then
      (cd "$demo_dir" && trunk build --release)
    else
      (cd "$demo_dir" && trunk build)
    fi
  fi
done
