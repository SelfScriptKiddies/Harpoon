#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
WEB_DIR="$PROJECT_ROOT/crates/harpoon-web"
OUTPUT_DIR="$PROJECT_ROOT/crates/harpoon-app/src/ui/web/static"

echo "Building Harpoon web UI (local)..."

cd "$WEB_DIR"

# Install deps if needed
if [ ! -d "node_modules" ]; then
  echo "Installing dependencies..."
  npm install
fi

# Build
npx vite build --outDir "$OUTPUT_DIR" --emptyOutDir

echo "Web UI built to $OUTPUT_DIR"
echo ""
echo "Now rebuild harpoon with: cargo build --features web"
