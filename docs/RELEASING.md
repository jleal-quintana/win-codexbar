---
summary: "Windows-local release checklist for Win-CodexBar using GitHub Releases and the local Setup.exe installer."
read_when:
  - Publishing a new Win-CodexBar release
  - Updating the local Windows packaging flow
  - Verifying auto-update assets and release notes
---

# Release Process (Win-CodexBar)

This repository ships the Rust Windows port. Releases are built locally and published with `gh`; GitHub Actions are not part of the active release path.

## Source Of Truth
- Runtime version: `rust/Cargo.toml`
- Release notes: the matching version section in `rust/CHANGELOG.md`
- Installable asset: `CodexBar-<version>-Setup.exe`
- Optional manual asset: `codexbar.exe`

## Prereqs
- Rust toolchain with the `x86_64-pc-windows-gnu` target
- MinGW-w64 linker toolchain
- Docker
- `gh` authenticated to `Finesssee/Win-CodexBar`

## Build Installer Locally
From the repository root:

```bash
cd rust
cargo build --release --bins
./scripts/build-installer.sh
```

This produces:
- `rust/target/x86_64-pc-windows-gnu/release/codexbar.exe`
- `rust/target/installer/CodexBar-<version>-Setup.exe`

## Publish A Release Locally
When `rust/Cargo.toml` and `rust/CHANGELOG.md` already match the target version:

```bash
./rust/scripts/release-local.sh <version>
```

For a prerelease:

```bash
./rust/scripts/release-local.sh <version> --prerelease
```

What the script does:
- fails if the git worktree is dirty
- verifies the requested version matches `rust/Cargo.toml`
- extracts release notes from the matching `rust/CHANGELOG.md` section
- runs `cargo clippy --all-targets -- -D warnings`
- runs `cargo check --quiet`
- runs `cargo test --no-run --quiet`
- runs `cargo build --release --bins`
- builds the Setup installer locally through Docker/Wine + Inno Setup
- creates or updates the GitHub release and uploads both assets

## Release Checklist
- [ ] Update `rust/Cargo.toml`
- [ ] Finalize the matching `rust/CHANGELOG.md` section
- [ ] Run `./rust/scripts/release-local.sh <version>`
- [ ] Confirm the GitHub release contains `CodexBar-<version>-Setup.exe`
- [ ] Confirm the GitHub release contains `codexbar.exe`
- [ ] Confirm GitHub shows SHA256 digests for both assets
- [ ] Install an older build under Wine or Windows, trigger the in-app updater, and verify `Restart & Update` launches the new installer

## Notes
- The updater prefers the `-Setup.exe` asset and only treats the plain `.exe` as a manual download fallback.
- The active installer flow is local and Windows-focused. The old macOS/Sparkle release story is historical and should not be used for this fork.
