#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: rust/scripts/release-local.sh <version> [--prerelease]

Builds the Windows release locally, creates the Setup installer, and creates or updates
the GitHub release for the matching tag using gh.
EOF
}

if [[ $# -lt 1 || $# -gt 2 ]]; then
  usage >&2
  exit 1
fi

VERSION="$1"
PRERELEASE=false
if [[ "${2:-}" == "--prerelease" ]]; then
  PRERELEASE=true
elif [[ $# -eq 2 ]]; then
  usage >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUST_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$RUST_DIR/.." && pwd)"
REPO_SLUG="Finesssee/Win-CodexBar"
TAG="v$VERSION"
PORTABLE_EXE="$RUST_DIR/target/x86_64-pc-windows-gnu/release/codexbar.exe"

cd "$REPO_ROOT"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "Release aborted: git worktree is dirty." >&2
  exit 1
fi

MANIFEST_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$RUST_DIR/Cargo.toml" | head -n 1)"
if [[ "$VERSION" != "$MANIFEST_VERSION" ]]; then
  echo "Version mismatch: requested $VERSION but rust/Cargo.toml is $MANIFEST_VERSION." >&2
  exit 1
fi

extract_release_notes() {
  awk -v version="$1" '
    $0 ~ "^## \\[" version "\\]" { capture = 1; next }
    capture && /^## \[/ { exit }
    capture { print }
  ' "$2"
}

NOTES_FILE="$(mktemp)"
cleanup() {
  rm -f "$NOTES_FILE"
}
trap cleanup EXIT

extract_release_notes "$VERSION" "$RUST_DIR/CHANGELOG.md" > "$NOTES_FILE"
if ! grep -q '[^[:space:]]' "$NOTES_FILE"; then
  echo "Could not find changelog notes for version $VERSION in rust/CHANGELOG.md." >&2
  exit 1
fi

(
  cd "$RUST_DIR"
  cargo clippy --all-targets -- -D warnings
  cargo check --quiet
  cargo test --no-run --quiet
  cargo build --release --bins
)

INSTALLER_PATH="$("$SCRIPT_DIR/build-installer.sh" "$VERSION")"

if [[ ! -f "$PORTABLE_EXE" ]]; then
  echo "Portable build artifact missing: $PORTABLE_EXE" >&2
  exit 1
fi

if gh release view "$TAG" --repo "$REPO_SLUG" >/dev/null 2>&1; then
  gh release upload "$TAG" \
    "$INSTALLER_PATH#CodexBar Installer" \
    "$PORTABLE_EXE#Portable EXE" \
    --clobber \
    --repo "$REPO_SLUG"

  EDIT_ARGS=(
    "$TAG"
    --repo "$REPO_SLUG"
    --title "$TAG"
    --notes-file "$NOTES_FILE"
    --latest
  )
  if [[ "$PRERELEASE" == true ]]; then
    EDIT_ARGS+=(--prerelease)
  fi
  gh release edit "${EDIT_ARGS[@]}"
else
  CREATE_ARGS=(
    "$TAG"
    "$INSTALLER_PATH#CodexBar Installer"
    "$PORTABLE_EXE#Portable EXE"
    --repo "$REPO_SLUG"
    --title "$TAG"
    --notes-file "$NOTES_FILE"
    --latest
  )
  if [[ "$PRERELEASE" == true ]]; then
    CREATE_ARGS+=(--prerelease)
  fi
  gh release create "${CREATE_ARGS[@]}"
fi

echo "Release $TAG is up to date."
