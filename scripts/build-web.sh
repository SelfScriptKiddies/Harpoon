#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUTPUT_DIR="$PROJECT_ROOT/crates/harpoon-app/src/ui/web/static"

echo "Building Harpoon web UI..."

# Build using Docker
CONTAINER_ID=$(docker build -q -f "$PROJECT_ROOT/docker/build-web.Dockerfile" "$PROJECT_ROOT")

# Extract built files from the image
TEMP_CONTAINER=$(docker create "$CONTAINER_ID")
rm -rf "$OUTPUT_DIR"
docker cp "$TEMP_CONTAINER:/app/dist" "$OUTPUT_DIR"
docker rm "$TEMP_CONTAINER" > /dev/null

echo "Web UI built to $OUTPUT_DIR"
ls -la "$OUTPUT_DIR"

echo ""
echo "Now rebuild harpoon with: cargo build --features web"
