#!/usr/bin/env bash
# Prepare a fresh checkout or worktree so it builds and agents have their skills.
#
#   1. Link the local-only files (originals/, TODO.md) from the main checkout.
#   2. Extract the game assets from originals/ski32.exe.
#   3. Restore agent skills from skills-lock.json.
#   4. Fetch the Rust dependencies.
#
# Safe to re-run. Intended as the Orca repo setup script: ./tools/setup.sh
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

warn() { printf '⚠️  %s\n' "$*" >&2; }
step() { printf '\n==> %s\n' "$*"; }

# The main checkout holds the gitignored files a new worktree doesn't have.
main="$(cd "$(git rev-parse --path-format=absolute --git-common-dir)/.." && pwd)"

step "Local-only files"
if [[ "$main" == "$root" ]]; then
  echo "Main checkout; nothing to link."
else
  for item in originals TODO.md; do
    if [[ -e "$root/$item" || -L "$root/$item" ]]; then
      echo "$item already present."
    elif [[ -e "$main/$item" ]]; then
      ln -s "$main/$item" "$root/$item"
      echo "Linked $item -> $main/$item"
    else
      warn "$item not found in $main; skipping."
    fi
  done
fi

step "Game assets"
if [[ -f originals/ski32.exe ]]; then
  python3 tools/extract_assets.py
else
  warn "originals/ski32.exe missing: the build will fail until you add it and run tools/extract_assets.py (see README.md)."
fi

step "Agent skills"
if command -v npx >/dev/null 2>&1; then
  npx -y skills experimental_install --agent claude-code codex --frozen-lockfile \
    || warn "Skill restore failed; run it again later: npx skills experimental_install"
else
  warn "npx not found; install Node.js to restore skills from skills-lock.json."
fi

step "Rust dependencies"
cargo fetch

step "Done"
