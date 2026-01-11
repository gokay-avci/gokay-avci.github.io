#!/bin/bash
set -e
echo "🧪 Starting Voxel Daisy Lab..."
rm -rf static/demos/porosity_lab
echo "🦀 Building WASM (Watch Mode)..."
(cd demos/porosity_lab && trunk watch) &
TRUNK_PID=$!
echo "⚡ Starting Zola (Port 1111)..."
trap "kill $TRUNK_PID" EXIT
zola serve --port 1111 --open