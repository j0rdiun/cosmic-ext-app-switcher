#!/usr/bin/env bash
# Regenerate cargo-sources.json after any Cargo.lock change.
# Requires: uv  (https://docs.astral.sh/uv/)
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
uv run --no-project --with aiohttp --with toml \
    flatpak/flatpak-cargo-generator.py Cargo.lock -o cargo-sources.json
echo "cargo-sources.json updated."
