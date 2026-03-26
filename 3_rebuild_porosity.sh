#!/bin/bash
set -euo pipefail
unset NO_COLOR
echo "🦀 Rebuilding porosity WASM demo..."
rm -rf static/demos/ai_porosity_puzzle public/demos/ai_porosity_puzzle
(cd demos/porosity_lab && trunk build)
echo "✅ Updated static/demos/ai_porosity_puzzle"
