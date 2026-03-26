#!/bin/bash
set -euo pipefail
unset NO_COLOR
BASE_URL="http://127.0.0.1:1111"
echo "🧪 Starting Voxel Daisy Lab..."
rm -rf static/demos/ai_porosity_puzzle public/demos/ai_porosity_puzzle
echo "🦀 Building WASM..."
(cd demos/porosity_lab && trunk build)
echo "⚡ Starting Zola (Port 1111)..."
echo "🌐 Local base URL: $BASE_URL"
zola serve --port 1111 --base-url "$BASE_URL" --no-port-append --open --store-html
