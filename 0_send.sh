#!/bin/bash

# A simple script to add, commit, and push all changes.

# Check if a commit message was provided as the first argument.
if [ -z "$1" ]; then
  echo "❌ Error: Please provide a commit message."
  echo "   Usage: ./publish.sh \"Your update message\""
  exit 1
fi

echo ">>> Staging all changes..."
git add .

echo ">>> Committing changes..."
git commit -m "$1"

echo ">>> Pushing to GitHub..."
git push

echo "✅ Done! Deployment triggered."