// Extrae el frontend original de Maestro Studio (dist/ dentro de app.asar), lo traduce
// al español, desactiva la telemetría y le inyecta el shim de Electron → studio-ui/.
//
// Uso: node scripts/build-studio-ui.mjs [ruta/a/app.asar]
// También acepta la variable de entorno MAESTRO_ASAR.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const OUT = path.join(ROOT, "studio-ui");
const DEFAULT_ASAR = "/Applications/Maestro Studio.app/Contents/Resources/app.asar";
const ASAR = process.argv[2] || process.env.MAESTRO_ASAR || DEFAULT_ASAR;

const MAIN_BUNDLE = "index-0XZa5ilO.js";
// Rangos (en caracteres) del bundle principal que contienen código de Maestro Studio.
// Entre STUDIO_1_END y STUDIO_2_START vive Monaco: no se traduce para no romperlo.
const STUDIO_1 = [490000, 865000];
const STUDIO_2_START = 4350000;

function readAsarHeader(fd) {
  const head = Buffer.alloc(16);
  fs.readSync(fd, head, 0, 16, 0);
  const headerSize = head.readUInt32LE(4);
  const jsonLen = head.readUInt32LE(12);
  const json = Buffer.alloc(jsonLen);
  fs.readSync(fd, json, 0, jsonLen, 16);
  return { header: JSON.parse(json.toString("utf8")), dataStart: 8 + headerSize };
}

function extractDir(fd, node, dataStart, outDir) {
  fs.mkdirSync(outDir, { recursive: true });
  for (const [name, entry] of Object.entries(node.files)) {
    const target = path.join(outDir, name);
    if (entry.files) {
      extractDir(fd, entry, dataStart, target);
      continue;
    }
    if (entry.unpacked) continue;
    const buf = Buffer.alloc(entry.size);
    fs.readSync(fd, buf, 0, entry.size, dataStart + Number(entry.offset));
    fs.writeFileSync(target, buf);
  }
}

function replaceAll(text, pairs) {
  let count = 0;
  for (const [from, to] of pairs) {
    const parts = text.split(from);
    count += parts.length - 1;
    text = parts.join(to);
  }
  return { text, count };
}

function mustReplace(text, from, to, label) {
  if (!text.includes(from)) {
    throw new Error(`Parche "${label}" no encontrado: ¿cambió la versión de Maestro Studio?`);
  }
  return text.split(from).join(to);
}

if (!fs.existsSync(ASAR)) {
  console.error(`No se encontró app.asar en ${ASAR}.`);
  console.error("Instala Maestro Studio (MaestroStudio.dmg) o pasa la ruta como argumento.");
  process.exit(1);
}

console.log(`Extrayendo UI original desde ${ASAR}`);
fs.rmSync(OUT, { recursive: true, force: true });
const fd = fs.openSync(ASAR, "r");
const { header, dataStart } = readAsarHeader(fd);
extractDir(fd, header.files.dist, dataStart, OUT);
fs.closeSync(fd);

const translations = JSON.parse(fs.readFileSync(path.join(ROOT, "translations_es.json"), "utf8"));
const pairs = Object.entries(translations.frontend);

// 1. Bundle principal: traducción, almacenamiento y telemetría
const bundlePath = path.join(OUT, "assets", MAIN_BUNDLE);
let js = fs.readFileSync(bundlePath, "utf8");
const before = js.slice(0, STUDIO_1[0]);
let studio1 = js.slice(STUDIO_1[0], STUDIO_1[1]);
const monaco = js.slice(STUDIO_1[1], STUDIO_2_START);
let studio2 = js.slice(STUDIO_2_START);

const t1 = replaceAll(studio1, pairs);
const t2 = replaceAll(studio2, pairs);
studio1 = t1.text;
studio2 = t2.text;
js = before + studio1 + monaco + studio2;
console.log(`Traducciones aplicadas: ${t1.count + t2.count}`);

// electron-store escribe con node:fs desde el renderer; se sustituye por un store del shim
js = mustReplace(js, "new Hae,", "new globalThis.CleanStudioStore,", "electron-store");
js = mustReplace(js, "new Hae;", "new globalThis.CleanStudioStore;", "electron-store");
// Sin cuenta: la bienvenida va directo a "Comenzar" (workspace), sin el paso de inicio de sesión
js = mustReplace(
  js,
  "a===0&&k(Van,{onNext:h})",
  "a===0&&k(Van,{onNext:()=>{d(1),l(3)}})",
  "onboarding sin login",
);
// …y el layout ya no reabre el asistente por no tener token de Maestro Cloud
js = mustReplace(js, "D.useEffect(()=>{!s&&!n&&c(!0)},[n,s])", "D.useEffect(()=>{},[n,s])", "gate de token");
// Monaco pide el worker principal como "editorWorkerService": sin esto cae al hilo principal
js = mustReplace(
  js,
  'case"editor-worker":return new Eln;',
  'case"editor-worker":case"editorWorkerService":return new Eln;',
  "worker de Monaco",
);
// PostHog: nunca capturar (además la CSP bloquea cualquier envío)
js = mustReplace(
  js,
  "disable_session_recording:!1",
  "disable_session_recording:!0,opt_out_capturing_by_default:!0",
  "posthog",
);
fs.writeFileSync(bundlePath, js);

// 2. Logo
const logo = `<svg width="595" height="142" viewBox="0 0 595 142" fill="none" xmlns="http://www.w3.org/2000/svg">
  <rect x="8" y="16" width="110" height="110" rx="28" fill="url(#cs_grad)"/>
  <path d="M44 72L62 90L94 52" stroke="#020617" stroke-width="12" stroke-linecap="round" stroke-linejoin="round"/>
  <text x="142" y="92" fill="currentColor" font-family="-apple-system, BlinkMacSystemFont, 'SF Pro Display', Inter, sans-serif" font-size="68" font-weight="800" letter-spacing="-1.5">Clean Studio</text>
  <defs>
    <linearGradient id="cs_grad" x1="8" y1="16" x2="118" y2="126" gradientUnits="userSpaceOnUse">
      <stop stop-color="#34D399"/>
      <stop offset="1" stop-color="#38BDF8"/>
    </linearGradient>
  </defs>
</svg>`;
fs.writeFileSync(path.join(OUT, "assets", "Maestro logo _ wordmark-CXPWZs15.svg"), logo);

// 3. index.html: título y shim antes del bundle (debe ejecutarse primero, de forma síncrona)
const htmlPath = path.join(OUT, "index.html");
let html = fs.readFileSync(htmlPath, "utf8");
html = html.replace("<html lang=\"en\">", "<html lang=\"es\">");
html = mustReplace(html, "Maestro Studio - Desktop", "Clean Studio", "título");
html = mustReplace(
  html,
  '<script type="module"',
  '<script src="./electron-shim.js"></script>\n    <script type="module"',
  "shim",
);
fs.writeFileSync(htmlPath, html);
fs.copyFileSync(path.join(ROOT, "src", "shim", "electron-shim.js"), path.join(OUT, "electron-shim.js"));

console.log(`UI lista en ${path.relative(ROOT, OUT)}/`);

// 4. Motores para empaquetar: resources/engines/ tiene symlinks hacia Maestro Studio.app,
//    que Tauri no copia. Se materializan una sola vez en engines-bundle/ (solo la JVM de esta arquitectura).
const ENGINES_SRC = path.join(ROOT, "resources", "engines");
const ENGINES_OUT = path.join(ROOT, "engines-bundle");
const jvmArch = process.arch === "arm64" ? "mac-arm64" : "mac-x64";
const engineParts = [
  "dist-server/studio-server.jar",
  `bundled-jvm/${jvmArch}`,
  "bundled-simulator-server/mac-universal",
];
for (const part of engineParts) {
  const target = path.join(ENGINES_OUT, part);
  if (fs.existsSync(target)) continue;
  const source = fs.realpathSync(path.join(ENGINES_SRC, part));
  console.log(`Copiando motor ${part}…`);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  // ditto conserva permisos, symlinks internos y firmas de la JVM
  execFileSync("ditto", [source, target], { stdio: "inherit" });
}
