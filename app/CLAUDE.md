# CLAUDE.md – hmb (Hugo Microblogger)

Rust desktop app: eframe/egui 0.32 (glow) on winit 0.30, macOS first (Apple Silicon).
UI texts are German. Code comments are English.

## Layout

- `src/main.rs` starts `eframe::run_native`, `src/app.rs` holds `HmbApp` and `update()`.
- `src/app/*.rs` contains `impl HmbApp` blocks per area (editor, images, panels, windows, widgets).
- `src/store.rs` handles articles on disk, `src/sync.rs` the transaction-based folder sync,
  `src/tray.rs` the menu bar icon (objc2).
- `release/make-dmg.sh` builds `dist/hmb-<version>-macos-<arch>.dmg` plus `.sha256`.
- `changelog.md` follows Keep a Changelog. The version lives in `Cargo.toml`.

## Rule: never run a modal dialog inside `update()`

Do not call blocking `rfd::FileDialog` (or anything that runs `NSApp runModal`) from `update()`.
The modal spins the run loop while the winit event handler is still active. A drag inside the
panel runs the loop in default mode, winit's queued `CFRunLoopPerformBlock` closure fires,
re-enters the handler and panics. The panic cannot unwind through an ObjC block, so the app
aborts (`panic_cannot_unwind`, SIGABRT, crash in 0.1.0).

Use `HmbApp::start_pick(ctx, PickKind, dir)` in `src/app/images.rs`. It runs
`rfd::AsyncFileDialog` with `pollster::block_on` in a thread and sends the result over a
channel. `poll_pick()` in `update()` applies it. For a new dialog, add a `PickKind` variant
and handle it in `poll_pick`. Inside `egui::Window` closures, only record the request and
call `start_pick` after the closure (see the settings window in `windows.rs`).

## Building the DMG

Run `./release/make-dmg.sh` (from anywhere, it changes into `app/`). It signs ad hoc unless
`SIGN_ID` is set, and builds universal if both Rust targets are installed.

- Do not use `hdiutil create -srcfolder`. Its temporary volume is mounted visibly, Spotlight or
  XProtect scan the fresh app, and the unmount fails with "Ressource ist belegt" (49168).
  The script creates a blank image, attaches it with `-nobrowse` at `dist/work/mnt`, copies
  with `ditto`, detaches (retry, then `-force`) and converts to UDZO.
- If it still fails, check `hdiutil info` for an old `hmb` DMG still mounted under `/Volumes`
  and detach it.

## Releasing to GitHub

Repo: `megaproaktiv/hugo-micro-blogging` (git root is the parent of `app/`). `dist/`, `target/`
and `Cargo.lock` are git-ignored.

1. Bump `version` in `Cargo.toml`, add a `changelog.md` entry, run `./release/make-dmg.sh`.
2. Commit, `git tag -a vX.Y.Z -m "hmb X.Y.Z"`, `git push origin main vX.Y.Z`.
3. `env -u GITHUB_TOKEN gh release create vX.Y.Z dist/hmb-X.Y.Z-macos-arm64.dmg dist/hmb-X.Y.Z-macos-arm64.dmg.sha256 --title "hmb X.Y.Z" --notes-file <notes>`

The `GITHUB_TOKEN` in the shell environment gets HTTP 403 for releases. `env -u GITHUB_TOKEN`
makes `gh` use the keyring login, which has `repo` scope. Release notes are German and follow
the v0.1.0 layout: heading "hmb X.Y.Z – Hugo Microblogger", install hint, list of changes.
