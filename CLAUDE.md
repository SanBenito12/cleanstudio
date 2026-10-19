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
cd src-tauri && cargo test --offline --lib   # translator tests (errors.rs)
"…/clean-studio" mcp                     # MCP server over stdio (no window)
```

The only tests are `errors.rs` unit tests plus an `#[ignore]`d `runs.rs` test that rebuilds the evidence from a saved SSE capture (`CS_SSE_SAMPLE=… CS_OUT=… cargo test --lib -- --ignored`). To debug the UI, run the binary directly (`"…/Clean Studio.app/Contents/MacOS/clean-studio" [workspace-dir]`) and read stderr. The shim forwards renderer `console.error`/`console.warn`, uncaught errors and rejections as `[RENDERER:level]` lines. Passing a directory argument opens it as the workspace, which skips the native folder picker.

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
     - Maestro Cloud UI removed: the Local/Cloud tabs, the Updates/Account settings sections, the logout footer (regex), promotional popups, cloud milestone toasts, the Maestro tour video
     - visible "Maestro" branding replaced (About window, welcome texts)
     - step labels and step errors routed through `globalThis.CleanStudioErrores.paso/explicar` (in `Zke`)
   - It injects `<script src="./electron-shim.js">` before the module script and prepends `src/shim/errores_es.json` to the shim as `globalThis.CLEAN_STUDIO_ERRORES`.
   - It copies the engines (symlinks in `resources/engines/` → real files) into `engines-bundle/` once, because Tauri doesn't bundle symlinks.
2. **`src/shim/electron-shim.js`** is a classic, synchronous script that runs before the bundle. It defines `window.require`:
   - `electron` → an `ipcRenderer` emulation;
   - `path`/`os`/`util`/`process` → small real implementations;
   - `fs`/`crypto`/`assert`/… → inert proxies, because the bundle only enumerates their exports at load time.
   
   It also defines `CleanStudioStore` (localStorage), opens external links with the system browser, maps `-webkit-app-region: drag` classes to `startDragging()`, and forwards console errors to Rust. It wraps `EventSource` to *observe* the `flowStatus`/`workspaceStatus` SSE streams and forwards every message to Rust as `cs-run-update`. It also defines `CleanStudioErrores`, the Spanish step/error translator. It requires `app.withGlobalTauri: true`.
3. **`src-tauri/src/`** reimplements the original `dist-electron/main.js`. Read that file (inside the asar) for exact semantics.
   - `ipc.rs` exposes two commands:
     - `ipc_send` = `ipcRenderer.send`
     - `ipc_invoke` = `ipcRenderer.invoke`
   - `ipc.rs` `match`es on the original channel names. Main→renderer messages use `bridge::send(app, channel, args)`, which emits one Tauri event `cs-ipc {channel, args}`. The shim dispatches it to `ipcRenderer.on(channel)` listeners as `(event, ...args)`. Payload shapes must match the original exactly (e.g. `server-status {running, port}`, `file-content-loaded {content, path}`, `directory-contents (path, items)`).
   - `server.rs` runs `studio-server.jar` on port 5050 using the bundled JVM. It kills orphaned servers that run our own jar path; it never touches a running Maestro Studio. `simulator.rs` runs `simulator-server <ios|android> --id <udid>`, parses `stream_ready … stream.mjpeg` and writes device commands to stdin.
   - `workspace.rs` handles the file tree (depth-2 preload plus expanded folders) and a polling watcher (no `notify` crate). `settings.rs` stores JSON in `app_config_dir()/config.json`. `windows.rs` holds the main and about windows, the menu and the fullscreen events. `bridge.rs` provides native dialogs via `osascript` (no dialog plugin). `paths.rs` resolves engines in this order: bundled resources → `resources/engines/` → `/Applications/Maestro Studio.app`.
   - `runs.rs` turns `cs-run-update` messages into run folders (`<workspace>/.clean-studio/ejecuciones/<ts>-<flow>/`). Each folder gets `ejecucion.json`, `reporte.html` (from `reports.rs`), the engine's own video and the screenshots. Never record the screen yourself: `simctl recordVideo` in parallel breaks the engine's recording, and the engine then crashes the flow. The engine writes the video to `~/.maestro/tests/<videoUrl>` and screenshots to `~/.maestro/tests/<session>/artifacts/takeScreenshot/`.
   - `errors.rs` compiles the same `errores_es.json` (regex, `include_str!`) for reports.
   - `mcp.rs` is the `clean-studio mcp` stdio JSON-RPC server. It drives studio-server over HTTP via `curl` and starts its own JVM when port 5050 is closed. The engine's built-in MCP is unusable because the jar lacks the MCP SDK. "Conectar con Claude…" in `windows.rs` writes `claude_desktop_config.json` or copies the `claude mcp add` command.
   - `lib.rs` handles SIGINT/SIGTERM (via `libc`) so child processes are cleaned up.
   - Cloud/OAuth/auto-update channels return neutral values or Spanish errors.

The renderer talks to `studio-server` directly over HTTP/SSE at `http://localhost:5050` (CORS `*`). Telemetry is blocked twice:
- The CSP in `tauri.conf.json` allows only localhost connections from the UI.
- `server::apply_no_telemetry` gives the JVM a dead HTTP(S) proxy (127.0.0.1:9; localhost exempt) plus `MAESTRO_CLI_NO_ANALYTICS=1`, because the jar hardcodes a Sentry DSN.

Use `apply_no_telemetry` for every JVM you spawn.

## Other directories

- `studio-ui/`, `engines-bundle/`, `src-tauri/target/`: generated, gitignored.
- `legacy/`: the previous hand-written React UI and its Rust commands; not built.
- `build_real_clean_studio.py`, `patched-studio-files/`: an older, separate track that repacks the Electron app itself.
- `packages/maestro-oss/`: a reference checkout of `mobile-dev-inc/maestro` (gitignored, has its own `CLAUDE.md`).
