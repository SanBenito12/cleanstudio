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
4. **Cero telemetría**, en la interfaz y en el motor:
   - **Interfaz**: la CSP solo permite conexiones a `localhost`, así que Sentry y PostHog no pueden enviar nada.
   - **Motor**: `studio-server.jar` trae Sentry con el DSN fijo y la analítica de Maestro CLI. La JVM arranca con `MAESTRO_CLI_NO_ANALYTICS=1` y todo su HTTP(S) saliente va a un proxy local inexistente; solo localhost queda directo. Verificado con `lsof` durante una ejecución fallida: ninguna conexión externa.
   - Maestro Cloud, OAuth y las actualizaciones automáticas están deshabilitados.

---

## Funciones de Clean Studio

- **Evidencia de cada ejecución**: al terminar una prueba se crea `<workspace>/.clean-studio/ejecuciones/<fecha>-<flujo>/` con:
  - el **video** que grabó el motor;
  - las capturas (`takeScreenshot`);
  - `ejecucion.json`;
  - un **reporte HTML en español** listo para compartir.
  
  En el menú **Ejecuciones** puedes abrir el último reporte o la carpeta, decidir si se guarda el video y si el reporte se abre al terminar.
- **Errores en español**: cada paso y cada error del motor se muestran traducidos, con una explicación y cómo corregirlo. La tabla vive en `src/shim/errores_es.json` y la comparten la interfaz y los reportes.
- **Claude controla Clean Studio (MCP)**: el botón **Claude** de la cabecera (o el menú **Clean Studio → Conectar con Claude…**) abre una ventana que indica si ya funciona, conecta Claude Desktop y Claude Code con un clic y trae ejemplos para pedirle a Claude. Usa tu suscripción de Claude, sin API key. Herramientas:

  | Herramienta | Qué hace |
  |---|---|
  | `listar_dispositivos`, `conectar_dispositivo` | Ver y encender simuladores o emuladores |
  | `ver_pantalla` | Captura de la pantalla más la lista de elementos (texto, id, posición) |
  | `ejecutar_comandos` | Ejecutar pasos YAML sueltos al instante |
  | `validar_flujo`, `ejecutar_flujo` | Validar o ejecutar un flujo; deja video y reporte |
  | `ultimas_ejecuciones`, `leer_ejecucion` | Consultar la evidencia guardada |
  | `guia_comandos` | Referencia de comandos YAML en español |

  Para Claude Code:

  ```bash
  claude mcp add --scope user clean-studio -- "/Applications/Clean Studio.app/Contents/MacOS/clean-studio" mcp
  ```
- **Sin nube ni cuentas**: no hay inicio de sesión, ni pestañas o avisos de Maestro Cloud, ni actualizaciones automáticas.

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
- `src/shim/errores_es.json`: traducciones de pasos y errores del motor.
- `src-tauri/src/`:
  - `ipc.rs`: despacho de canales.
  - `server.rs` y `simulator.rs`: los motores.
  - `runs.rs`, `reports.rs` y `errors.rs`: evidencia, reportes y traductor.
  - `mcp.rs`: servidor MCP para Claude.
  - `workspace.rs`, `settings.rs` y `windows.rs`.
  - `bridge.rs` y `paths.rs`: utilidades compartidas.
- `sample-flows/`: flujos YAML de ejemplo en español.
- `legacy/`: la interfaz React anterior, como referencia.
- `build_real_clean_studio.py`: variante que reempaqueta la app Electron original traducida.
