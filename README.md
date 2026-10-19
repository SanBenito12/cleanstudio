# Clean Studio (`clean-studio`)

**Automatización E2E móvil (Android / iOS) y web en español • Núcleo nativo en Rust (Tauri v2) + motores de Maestro • Sin cuentas • Cero telemetría**

---

## Cómo funciona

1. **Interfaz** (`studio-ui/`, generada): `scripts/build-studio-ui.mjs` toma la interfaz de Maestro Studio 0.9.8 instalada en `/Applications`. Sobre ella:
   - la traduce al español (`translations_es.json`);
   - quita el inicio de sesión obligatorio de Maestro Cloud;
   - desactiva PostHog;
   - le inyecta `src/shim/electron-shim.js`.
2. **Shim de Electron** (`src/shim/electron-shim.js`): la interfaz espera ejecutarse en Electron. El shim le da `require("electron").ipcRenderer` y los módulos de Node que carga al arrancar, y manda cada canal IPC al backend Rust.
3. **Núcleo nativo** (`src-tauri/`): reemplaza el proceso principal de Electron.
   - `studio-server.jar` (puerto `5050`), con su JVM empaquetada.
   - `simulator-server`, el espejo en vivo de iOS y Android vía MJPEG.
   - Workspace y archivos: árbol, vigilancia de cambios, crear, renombrar, mover, duplicar y enviar a la Papelera.
   - Ajustes (Android SDK, editor), ventanas, menú nativo y diálogos de macOS.
4. **Cero telemetría**: la CSP solo permite conexiones a `localhost`, así que Sentry y PostHog no pueden enviar nada. Maestro Cloud, OAuth y las actualizaciones automáticas están deshabilitados.

---

## Requisitos

- macOS con **Maestro Studio 0.9.8** instalado en `/Applications`, de donde se toman la interfaz y los motores (`MaestroStudio.dmg`).
- Node.js y Rust (cargo).
- Xcode (simuladores iOS) y/o Android SDK (emuladores).

---

## Comandos

```bash
# Prepara studio-ui/ y engines-bundle/ (lo ejecutan también los comandos de Tauri)
npm run studio-ui

# Desarrollo con ventana nativa
npm run tauri:dev

# Compilar la app (.app / .dmg, incluye JVM + studio-server + simulator-server)
npm run tauri:build
```

Para abrir directamente una carpeta como workspace:

```bash
"Clean Studio.app/Contents/MacOS/clean-studio" ~/ruta/a/mis-flujos
```

---

## Estructura

- `scripts/build-studio-ui.mjs`: extrae, traduce y parchea la interfaz, y prepara `engines-bundle/`.
- `src/shim/electron-shim.js`: emulación de Electron dentro del webview.
- `src-tauri/src/`:
  - `ipc.rs`: despacho de canales.
  - `server.rs` y `simulator.rs`: los motores.
  - `workspace.rs`, `settings.rs` y `windows.rs`.
  - `bridge.rs` y `paths.rs`: utilidades compartidas.
- `sample-flows/`: flujos YAML de ejemplo en español.
- `legacy/`: la interfaz React anterior, como referencia.
- `build_real_clean_studio.py`: variante que reempaqueta la app Electron original traducida.
