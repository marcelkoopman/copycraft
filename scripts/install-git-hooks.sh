#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

chmod +x .githooks/pre-commit .githooks/pre-push
git config core.hooksPath .githooks
echo "Git hooks path set to .githooks"
echo "  pre-commit  cargo fmt --all"
echo "  pre-push    cargo clippy -- -D warnings && cargo test"
