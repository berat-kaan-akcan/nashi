# Nashi for Linux — agent instructions

Linux (KDE Plasma 6 Wayland first, CachyOS) port of the Nashi drag + modifier + wheel file converter. Owner: Berat (communicates in Turkish; user-facing docs like `docs/PLAN.md` and README summaries in Turkish, code/comments/commits in English).

Read `docs/PLAN.md`, `docs/ARCHITECTURE.md`, and the contracts `docs/CLI-PROTOCOL.md` and `docs/DBUS.md` before changing code. Contracts are shared between components — if you change one, update every side in the same change.

## Layout and commands

- Rust workspace: `crates/nashi-core`, `crates/nashi-cli`, `crates/nashi-inputd`
  - `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all`
- Qt6/QML app: `ui/`
  - `cmake -S ui -B build/ui -G Ninja && cmake --build build/ui && ctest --test-dir build/ui --output-on-failure`
  - Headless: `QT_QPA_PLATFORM=offscreen`
  - LayerShellQt is optional at build time (`NASHI_WITH_LAYERSHELL`, auto-detected); without it the overlay falls back to a frameless full-screen window.
- Packaging: `packaging/` (Arch `PKGBUILD`, `install.sh`), desktop files in `data/`.

## Rules

- Never modify, overwrite, move or delete user input files. Outputs: unique names next to the input, temp file + atomic rename.
- External tools: argv only, never through a shell. Guard file names that start with `-`.
- Drops onto the overlay are always accepted as `Qt::CopyAction`.
- `nashi-inputd` must never log key codes or text; only chord/drag state.
- Missing external tools must degrade gracefully (report, don't panic).
- Keep the catalog in sync with `reference/upstream-FeatureCatalog.json` (upstream Nashi, MIT — keep attribution in `NOTICE`).
- Tests: every engine route gets a test that is skipped (not failed) when its external tool is missing.
