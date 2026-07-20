#!/usr/bin/env sh
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
dist_dir=${EVOKUPS_DIST_DIR:-"$project_root/dist"}
profile=${EVOKUPS_PROFILE:-release}

case "$profile" in
  release)
    cargo_profile_args="--release"
    artifact_profile="release"
    ;;
  dev)
    cargo_profile_args=""
    artifact_profile="debug"
    ;;
  *)
    echo "EVOKUPS_PROFILE must be 'release' or 'dev'." >&2
    exit 2
    ;;
esac

# shellcheck disable=SC2086 # Empty in dev mode; one flag in release mode.
cargo build \
  --manifest-path "$project_root/Cargo.toml" \
  -p evokups-web \
  --target wasm32-unknown-unknown \
  $cargo_profile_args

mkdir -p "$dist_dir/web"
cp "$project_root/index.html" "$dist_dir/index.html"
cp "$project_root/web/app.js" "$dist_dir/web/app.js"
cp "$project_root/web/structure-view.js" "$dist_dir/web/structure-view.js"
cp "$project_root/web/periodic-geometry.js" "$dist_dir/web/periodic-geometry.js"
cp "$project_root/web/styles.css" "$dist_dir/web/styles.css"
cp \
  "$project_root/target/wasm32-unknown-unknown/$artifact_profile/evokups_web.wasm" \
  "$dist_dir/web/evokups_web.wasm"

echo "Static site assembled in $dist_dir"
