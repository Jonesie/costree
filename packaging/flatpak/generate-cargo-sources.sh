#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
#
# Regenerates packaging/flatpak/cargo-sources.json from Cargo.lock, using
# flatpak-cargo-generator.py from flatpak/flatpak-builder-tools. Flatpak
# builds run fully offline, so every crate (including our ~10 git-sourced
# libcosmic/iced/winit forks) needs to be pre-fetched into a sources list
# flatpak-builder downloads before disabling network for the actual build.
#
# Run this whenever Cargo.lock changes in a way that affects dependencies —
# treat cargo-sources.json like a second lockfile that needs to stay in
# sync, not a one-time generated artifact.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
generator_url="https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py"
venv_dir="$(mktemp -d)"
trap 'rm -rf "$venv_dir"' EXIT

python3 -m venv "$venv_dir"
"$venv_dir/bin/pip" install --quiet "aiohttp<4.0.0,>=3.9.5" "PyYAML<7.0.0,>=6.0.2" "tomlkit>=0.13.3,<1.0"

generator_script="$venv_dir/flatpak-cargo-generator.py"
curl -sSL -o "$generator_script" "$generator_url"

"$venv_dir/bin/python" "$generator_script" \
    "$repo_root/Cargo.lock" \
    -o "$repo_root/packaging/flatpak/cargo-sources.json"

echo "Wrote packaging/flatpak/cargo-sources.json"
