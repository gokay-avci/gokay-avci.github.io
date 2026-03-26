#!/bin/bash
set -euo pipefail
unset NO_COLOR
echo "🏗️  Building Production..."
rm -rf public static/demos
echo "🦀 Compiling Rust Demos..."
(cd demos/porosity_lab && trunk build --release)
(cd demos/grains && trunk build --release)
echo "⚡ Generating Static Site..."
zola build
echo "✅ Build Complete in /public"
