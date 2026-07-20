#!/usr/bin/env bash
set -euo pipefail

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$repo_root"

if [ "$#" -eq 0 ] || [ -z "$1" ]; then
  echo "❌ Error: Please provide a commit message." >&2
  echo "   Usage: ./0_send.sh \"Your update message\"" >&2
  exit 1
fi

echo ">>> Staging all changes..."
git add --all

if git diff --cached --quiet; then
  echo "ℹ️  Nothing to commit."
  exit 0
fi

echo ">>> Committing changes..."
git commit -m "$*"

echo ">>> Pushing to GitHub..."
git push

echo "✅ Done! Deployment triggered."
