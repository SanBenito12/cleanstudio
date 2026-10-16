# Clean Studio (`clean-studio`)

**Automatización E2E Móvil (Android / iOS) y Web en Español • Núcleo Nativo en Rust (Tauri v2) + Maestro Core • Cero Telemetría**

---

## Arquitectura (Ingeniería Inversa con REA)

`Clean Studio` fue diseñado tras analizar `MaestroStudio.dmg` (v0.9.8) con **REA (`morluto/rea`)** y el repositorio oficial [`mobile-dev-inc/maestro`](https://github.com/mobile-dev-inc/maestro):

1. **Núcleo Nativo en Rust (`src-tauri/`)**:
   - Reemplaza los 477 MB de `node_modules` de Electron por un orquestador ultraligero en **Rust + Tauri v2**.
   - Controla el ciclo de vida de `studio-server.jar` (puerto `5050`) y `simulator-server` (streaming MJPEG a 60 FPS).
   - Detecta automáticamente simuladores de iOS (`xcrun simctl`) y dispositivos Android (`adb`).
2. **Interfaz 100% en Español (`src/`)**:
   - **Constructor Visual de Pasos YAML**: Inserta comandos (`tapOn`, `inputText`, `assertVisible`, `scrollUntilVisible`, `launchApp`, `takeScreenshot`) con un clic.
   - **Espejo en Vivo e Inspector de Jerarquía (`ViewHierarchy`)**: Selecciona elementos de la pantalla del dispositivo para generar sus selectores automáticamente.
   - **Traductor Inteligente de Errores de Maestro**: Convierte excepciones técnicas del backend JVM en explicaciones claras en español.
   - **Asistente IA Libre (Offline Rust / Ollama Local / Gemini / OpenAI)**: Genera flujos YAML comentados en español sin requerir cuenta ni suscripción a Maestro Cloud.
   - **Exportador de Reportes HTML Locales**: Genera reportes ejecutivos autocontenidos en español en la carpeta `reports/`.
3. **Cero Telemetría**:
   - Eliminados al 100% `posthog-js`, `posthog-node`, `@sentry/electron` y todos los banners publicitarios de Maestro Cloud.

---

## Estructura del Proyecto

- `src-tauri/` — Backend nativo en **Rust (Tauri v2)** (`models.rs`, `engines.rs`, `ai_reports.rs`, `lib.rs`).
- `src/` — Frontend en **React 18 + TypeScript + Tailwind CSS v4** (`App.tsx`, `i18n/es.ts`, `services/bridge.ts`, `components/`).
- `resources/engines/` — Motores locales extraídos (`studio-server.jar`, `bundled-jvm`, `bundled-simulator-server`).
- `packages/maestro-oss/` — Clon completo del repositorio abierto `mobile-dev-inc/maestro`.
- `sample-flows/` — Flujos YAML de ejemplo en español listos para ejecutar.
- `reports/` — Reportes HTML generados localmente.

---

## Comandos Rápidos

```bash
# Modo desarrollo con ventana nativa de macOS (Rust + Tauri v2)
npm run tauri:dev

# Modo desarrollo web rápido (http://localhost:1420)
npm run dev

# Compilar aplicación nativa (.app / .dmg)
npm run tauri:build
```
