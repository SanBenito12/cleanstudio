# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Clean Studio is a Spanish-language, account-free, telemetry-free desktop app for Maestro (mobile/web E2E testing). It runs the **original Maestro Studio 0.9.8 renderer** (taken from `/Applications/Maestro Studio.app/Contents/Resources/app.asar`) inside a Tauri v2 window. A Rust backend replaces Electron's main process. Keep new user-facing strings and Rust error/log messages in Spanish.

## Commands

```bash
npm run studio-ui        # node scripts/build-studio-ui.mjs → studio-ui/ (+ engines-bundle/ on first run)
npm run tauri:dev        # runs studio-ui first, then the native window
npm run tauri:build      # .app + .dmg (bundles engines-bundle/ as resources; ~460 MB)
npx tauri build --debug --bundles app   # faster debug .app, used for manual testing
cd src-tauri && cargo check --offline   # quick Rust check
```

There are no tests or linters. To debug the UI, run the binary directly (`"…/Clean Studio.app/Contents/MacOS/clean-studio" [workspace-dir]`) and read stderr. The shim forwards renderer `console.error`/`console.warn`, uncaught errors and rejections as `[RENDERER:level]` lines. Passing a directory argument opens it as the workspace, which skips the native folder picker.

## Architecture

Three pieces must stay in sync:

1. **`scripts/build-studio-ui.mjs`** parses `app.asar` (no dependencies), extracts `dist/` to `studio-ui/`, and patches the main bundle `index-0XZa5ilO.js`:
   - It applies `translations_es.json["frontend"]` only to the character ranges outside Monaco (hardcoded offsets).
   - It applies exact-string patches through `mustReplace`, which fails loudly when Maestro changes:
     - `new Hae` (electron-store, which writes with `node:fs`) → `new globalThis.CleanStudioStore`
     - the onboarding skips the Maestro Cloud sign-in step
     - the layout no longer reopens onboarding when there's no auth token
     - Monaco's `editorWorkerService` worker label
     - PostHog opt-out
   - It injects `<script src="./electron-shim.js">` before the module script.
   - It copies the engines (symlinks in `resources/engines/` → real files) into `engines-bundle/` once, because Tauri doesn't bundle symlinks.
2. **`src/shim/electron-shim.js`** is a classic, synchronous script that runs before the bundle. It defines `window.require`:
   - `electron` → an `ipcRenderer` emulation;
   - `path`/`os`/`util`/`process` → small real implementations;
   - `fs`/`crypto`/`assert`/… → inert proxies, because the bundle only enumerates their exports at load time.
   
   It also defines `CleanStudioStore` (localStorage), opens external links with the system browser, maps `-webkit-app-region: drag` classes to `startDragging()`, and forwards console errors to Rust. It requires `app.withGlobalTauri: true`.
3. **`src-tauri/src/`** reimplements the original `dist-electron/main.js`. Read that file (inside the asar) for exact semantics.
   - `ipc.rs` exposes two commands:
     - `ipc_send` = `ipcRenderer.send`
     - `ipc_invoke` = `ipcRenderer.invoke`
   - `ipc.rs` `match`es on the original channel names. Main→renderer messages use `bridge::send(app, channel, args)`, which emits one Tauri event `cs-ipc {channel, args}`. The shim dispatches it to `ipcRenderer.on(channel)` listeners as `(event, ...args)`. Payload shapes must match the original exactly (e.g. `server-status {running, port}`, `file-content-loaded {content, path}`, `directory-contents (path, items)`).
   - `server.rs` runs `studio-server.jar` on port 5050 using the bundled JVM. It kills orphaned servers that run our own jar path; it never touches a running Maestro Studio. `simulator.rs` runs `simulator-server <ios|android> --id <udid>`, parses `stream_ready … stream.mjpeg` and writes device commands to stdin.
   - `workspace.rs` handles the file tree (depth-2 preload plus expanded folders) and a polling watcher (no `notify` crate). `settings.rs` stores JSON in `app_config_dir()/config.json`. `windows.rs` holds the main and about windows, the menu and the fullscreen events. `bridge.rs` provides native dialogs via `osascript` (no dialog plugin). `paths.rs` resolves engines in this order: bundled resources → `resources/engines/` → `/Applications/Maestro Studio.app`.
   - `lib.rs` handles SIGINT/SIGTERM (via `libc`) so child processes are cleaned up.
   - Cloud/OAuth/auto-update channels return neutral values or Spanish errors.

The renderer talks to `studio-server` directly over HTTP/SSE at `http://localhost:5050` (CORS `*`). The CSP in `tauri.conf.json` allows only localhost connections, and that's what blocks telemetry.

## Other directories

- `studio-ui/`, `engines-bundle/`, `src-tauri/target/`: generated, gitignored.
- `legacy/`: the previous hand-written React UI and its Rust commands; not built.
- `build_real_clean_studio.py`, `patched-studio-files/`: an older, separate track that repacks the Electron app itself.
- `packages/maestro-oss/`: a reference checkout of `mobile-dev-inc/maestro` (gitignored, has its own `CLAUDE.md`).
