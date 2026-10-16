# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Clean Studio is a Spanish-language, telemetry-free desktop front end for Maestro (mobile/web E2E testing). It doesn't reimplement Maestro. It orchestrates Maestro's prebuilt engines (`studio-server.jar` on port 5050 and the `simulator-server` MJPEG streamer) from a Tauri v2 (Rust) shell with a React 18 + TypeScript + Tailwind v4 UI. All user-facing text, error messages, logs and generated YAML comments are in Spanish. Keep new UI strings and Rust error strings in Spanish too.

The repo has **two independent tracks**:

1. **Tauri app** (`src/`, `src-tauri/`): the actual Clean Studio application.
2. **Electron repack** (`build_real_clean_studio.py`, `translations_es.json`, `patched-studio-files/`): a separate script that clones `/Applications/Maestro Studio.app` into `~/Downloads/Clean Studio.app`. It string-replaces translations into the bundled JS, turns PostHog opt-out on, rewrites `app.asar` (appending patched files and recomputing per-file integrity plus the header hash in `Info.plist` `ElectronAsarIntegrity`), then ad-hoc codesigns the result. It slices `index-0XZa5ilO.js` at hardcoded byte offsets so the Monaco section isn't touched by the translations, which means it only works for that exact Maestro Studio build (v0.9.8). It reads source files from a hardcoded `EXTRACTED_DIR` outside the repo.

## Commands

```bash
npm run dev            # Vite only, http://localhost:1420 (browser mode; see "Dual runtime" below)
npm run tauri:dev      # Native window; runs `npm run dev` first
npm run tauri:build    # .app + .dmg; runs `npm run build` (tsc + vite build) first
npm run build          # Type-check (tsc, noEmit) + Vite build into dist/
cd src-tauri && cargo check   # Check the Rust side quickly
python3 build_real_clean_studio.py   # Electron repack track (needs Maestro Studio.app installed)
```

There's no test suite or linter configured for this project. `tsc` (strict mode) is the only static check on the frontend.

## Architecture

### Frontend ↔ Rust bridge

- `src/services/bridge.ts` is the only place the UI talks to the backend. Each function calls a Tauri command through `invoke(...)`, and every one of those commands has to be registered in `tauri::generate_handler![...]` in `src-tauri/src/lib.rs`. To add a command: write the `#[tauri::command]` in `engines.rs` or `ai_reports.rs`, register it in `lib.rs`, then wrap it in `bridge.ts`. Tauri maps camelCase JS argument keys to snake_case Rust parameters (for example `deviceId` → `device_id`). Struct payloads go in as `{ req: ... }`.
- Shared serde types live in `src-tauri/src/models.rs`, and `bridge.ts` mirrors them as TS interfaces with the same snake_case field names. `AppState` keeps the spawned `studio-server` and simulator child processes plus a shared log buffer. Server stdout/stderr also goes out as `server-log` Tauri events.

### Dual runtime (important)

`bridge.ts` checks `isTauriRuntime()`. In plain-browser mode (`npm run dev`) most functions return **hardcoded mock data**: fake devices, sample-flow paths, a canned AI flow, and fake success messages. `executeCommandOnMaestroServer` POSTs straight to `http://localhost:5050/api/devices/runCommand` and **treats network failures as success**. `fetchLocalDevices` falls back to mock devices even inside Tauri when nothing is detected. So a passing run in the UI doesn't prove anything actually executed. Check against a real device and server.

### Rust backend (`src-tauri/src/`)

- `engines.rs`: resolves engine binaries under `resources/engines/` (`dist-server/studio-server.jar`, `bundled-jvm/mac-arm64/.../java`, `bundled-simulator-server/mac-universal/simulator-server`). If those are missing it falls back to the copies inside `/Applications/Maestro Studio.app/.../app.asar.unpacked/`. It runs `java -jar studio-server.jar`, injecting `ANDROID_HOME`/`PATH`, and detects devices with `xcrun simctl` and `adb devices -l`. Server status means "is port 5050 open", so an externally started server also counts as running.
- `ai_reports.rs`: workspace file CRUD for YAML flows, seeding of `sample-flows/` at startup (`ensure_sample_flows_internal`, called from `run()`), AI flow generation (Ollama at `127.0.0.1:11434`, OpenAI, Gemini, with an offline template fallback), and self-contained HTML report export to `reports/`.
- `base_project_dir()` is **hardcoded** to `/Users/ilove/Downloads/clean-studio` and falls back to the current working directory. Logs, reports, sample flows and engine lookup all hang off it. Several mock paths in `bridge.ts` are hardcoded the same way.

### Frontend (`src/`)

- `App.tsx` holds most of the state: the open flow, YAML editor (Monaco), step-by-step run loop, and diagnostics polling. It composes `SidebarWorkspace`, `DeviceMirrorInspector` (live mirror plus a view-hierarchy picker that generates selectors; it uses `DEFAULT_UI_ELEMENTS` mock nodes from `bridge.ts`) and `AiAssistantModal`.
- `src/i18n/es.ts` holds the Maestro command catalog (`MAESTRO_COMMANDS_ES`, with the snippets the step builder inserts), the flow templates (`FLOW_TEMPLATES_ES`, also used as browser-mode file contents), and the Spanish translations of Maestro errors.

## Other directories

- `packages/maestro-oss/`: a vendored clone of upstream `mobile-dev-inc/maestro` (Kotlin/Gradle), for reference only. Nothing here builds or imports it. It has its own `CLAUDE.md`/`AGENTS.md`, which apply when working inside it.
- `resources/engines/`: extracted prebuilt binaries (large; don't edit). Vite's watcher ignores `src-tauri/`, `packages/` and `resources/`.
- `.clean-studio-home/`: runtime home dir (`.maestro`, `.android`, server log) created when the engines run.
- `dist/`, `src-tauri/target/`, `reports/`: generated output.
