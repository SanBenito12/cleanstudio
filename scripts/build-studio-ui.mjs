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
// Logo de la bienvenida (componente Ban): el símbolo "Flujo" de Clean Studio en lugar de la marca de Maestro
const banLogo = /(function Ban\(\{className:i\}\)\{return k\("svg",\{className:i,viewBox:"0 25 92 92"[^}]*?k\("path",\{d:")[^"]+(")/;
if (!banLogo.test(js)) throw new Error('Parche "logo de bienvenida" no encontrado');
js = js.replace(banLogo, "$1M11.04 29.6H80.96A11.04 11.04 0 0 1 80.96 51.68H11.04A11.04 11.04 0 0 1 11.04 29.6ZM11.04 59.96H57.04A11.04 11.04 0 0 1 57.04 82.04H11.04A11.04 11.04 0 0 1 11.04 59.96ZM11.04 90.32H33.12A11.04 11.04 0 0 1 33.12 112.4H11.04A11.04 11.04 0 0 1 11.04 90.32ZM69.92 101.36a11.04 11.04 0 1 0 22.08 0a11.04 11.04 0 1 0 -22.08 0Z$2");
// Ventana "Acerca de": nombre y versión de Clean Studio (la fila "Maestro" sigue mostrando el motor)
const appVersion = JSON.parse(fs.readFileSync(path.join(ROOT, "package.json"), "utf8")).version;
js = mustReplace(
  js,
  'alt:"Maestro Studio",className:"w-full h-full rounded-xl"',
  'alt:"Clean Studio",className:"w-full h-full rounded-xl"',
  "Acerca de: icono",
);
js = mustReplace(
  js,
  'className:"text-lg font-semibold text-foreground",children:"Maestro Studio"',
  'className:"text-lg font-semibold text-foreground",children:"Clean Studio"',
  "Acerca de: título",
);
js = mustReplace(
  js,
  'children:"Maestro Studio"}),k("span",{className:"text-sm font-mono text-foreground",children:i||"Loading..."})',
  `children:"Clean Studio"}),k("span",{className:"text-sm font-mono text-foreground",children:"${appVersion}"})`,
  "Acerca de: versión",
);
// Sin nube ni marca de Maestro visible. Cada par es [original, reemplazo, etiqueta].
const UI_PATCHES = [
  // Panel de ejecución: sin selector Local/Cloud (la pestaña "cloud" no tiene forma de abrirse)
  [
    'X(FVe,{children:[k(Uk,{value:"local",children:"Local"}),k(Uk,{value:"cloud",children:"Cloud"})]})',
    'k("span",{className:"text-sm font-medium text-foreground",children:"Ejecuciones"})',
    "panel: sin Cloud",
  ],
  // Ajustes: sin secciones de actualizaciones ni de cuenta
  [
    '{id:"emulators",label:"Emulators",icon:mOe},{id:"updates",label:"Updates",icon:u1t},{id:"account",label:"Account",icon:E1t}]',
    '{id:"emulators",label:"Emuladores",icon:mOe}]',
    "ajustes: sin cuenta",
  ],
  [
    'k(Ll,{value:"system",children:"System"}),k(Ll,{value:"dark",children:"Dark"}),k(Ll,{value:"light",children:"Light"})',
    'k(Ll,{value:"system",children:"Sistema"}),k(Ll,{value:"dark",children:"Oscuro"}),k(Ll,{value:"light",children:"Claro"})',
    "ajustes: tema",
  ],
  // Popups promocionales de la nube: siempre apagados
  [
    "const r=nZ(s),o=(r==null?void 0:r.orgStatus)===Vr.ACTIVE;n(!o)",
    "n(!1)",
    "popups promocionales",
  ],
  // Bienvenida: sin el video de Maestro y con textos propios
  [
    'k(SR,{className:"my-2"}),k(UY,{icon:w1t,title:"Hacer recorrido",description:"Mira un video corto sobre cómo usar Clean Studio",onClick:e})',
    "null",
    "bienvenida: sin video",
  ],
  [
    `description:'Create a new folder for Maestro tests (often called "Maestro")'`,
    'description:"Crea una carpeta nueva para guardar tus pruebas"',
    "bienvenida: nuevo workspace",
  ],
  ['v=a||"No Workspace"', 'v=a||"Sin espacio de trabajo"', "cabecera: sin workspace"],
  // "Acerca de": la fila del motor
  [
    'k("span",{className:"text-sm text-muted-foreground",children:"Maestro"}),qO?',
    'k("span",{className:"text-sm text-muted-foreground",children:"Motor"}),qO?',
    "acerca de: motor",
  ],
];
for (const [from, to, label] of UI_PATCHES) js = mustReplace(js, from, to, label);

// Cabecera: botón "Claude" junto al selector de tema (abre "Conectar con Claude…" del backend)
const sparkRays = Array.from({ length: 8 }, (_, i) => {
  const a = (i * Math.PI) / 4;
  const r1 = i % 2 ? 3.2 : 2.6;
  const r2 = i % 2 ? 8.2 : 10;
  const p = (r) => [12 + r * Math.cos(a), 12 + r * Math.sin(a)].map((n) => n.toFixed(2));
  const [x1, y1] = p(r1);
  const [x2, y2] = p(r2);
  return `k("line",{x1:${x1},y1:${y1},x2:${x2},y2:${y2}})`;
}).join(",");
const claudeButton =
  'k(xt,{variant:"ghost",size:"sm",className:"gap-1.5 px-2.5 font-medium",' +
  'title:"Conectar con Claude: deja que Claude vea tus dispositivos, escriba y ejecute pruebas",' +
  '"aria-label":"Conectar con Claude",onClick:()=>ur("cs-connect-claude"),children:[' +
  'k("svg",{viewBox:"0 0 24 24",width:16,height:16,"aria-hidden":!0,' +
  `children:k("g",{stroke:"#D97757",strokeWidth:2.4,strokeLinecap:"round",children:[${sparkRays}]})}),` +
  'k("span",{children:"Claude"})]})';
js = mustReplace(
  js,
  'children:[k(NBe,{}),e&&k(xt,{variant:"ghost",size:"icon-sm",onClick:e,className:"text-amber-500 hover:text-amber-400"',
  `children:[${claudeButton},k(NBe,{}),e&&k(xt,{variant:"ghost",size:"icon-sm",onClick:e,className:"text-amber-500 hover:text-amber-400"`,
  "cabecera: botón Claude",
);
// Pasos y errores del panel de ejecución pasan por el traductor del shim (CleanStudioErrores)
js = mustReplace(
  js,
  "return{label:i.description??i.command??",
  "return{label:globalThis.CleanStudioErrores.paso(i.description??i.command)??",
  "traductor: pasos",
);
js = mustReplace(
  js,
  "commandId:i.id,errorMessage:i.error,",
  "commandId:i.id,errorMessage:globalThis.CleanStudioErrores.explicar(i.error),",
  "traductor: errores",
);
// Ajustes: sin el pie con "Cerrar Sesión" (no hay cuenta) ni su diálogo de confirmación
const logoutFooter =
  /k\(\$Qt,\{className:"p-\(--spacing-panel\) pt-0",children:X\(Fb,[\s\S]{0,900}?children:"Cerrar Sesión"\}\)\]\}\)\]\}\)\]\}\)\}\)/;
if (!logoutFooter.test(js)) throw new Error('Parche "ajustes: sin cerrar sesión" no encontrado');
js = js.replace(logoutFooter, "null");

// Avisos de hitos: sin promoción de la nube ni enlaces externos, en español
js = mustReplace(
  js,
  'description:"You\'ve successfully run your first flow. Now you can start building your test suite. Did you know you can run your entire workspace at once?",cta:{label:"Ver documentación",href:"https://maestro.mobile.dev"}}',
  'description:"Tu primer flujo se ejecutó bien. Ya puedes armar tu batería de pruebas; también puedes ejecutar todo el espacio de trabajo a la vez."}',
  "hito: primer flujo",
);
js = mustReplace(
  js,
  'title:"Let\'s get you back on track!",description:"Your flow ran into an error. Check out the test details to troubleshoot, or visit our documentation for tips. Feeling stuck? Our Slack community is full of developers who\'ve solved similar issues.",cta:{label:"Join our Slack",href:"https://slack.maestro.dev/"}}',
  'title:"El flujo encontró un error",description:"Revisa el detalle del paso que falló en el panel de ejecuciones: ahí verás la explicación en español y cómo corregirlo."}',
  "hito: primer error",
);
js = mustReplace(
  js,
  'description:"You\'ve successfully run your entire workspace. Ready to take it to the next level? Upload your workspace to Maestro Cloud to run tests on remote devices with faster execution and better reliability."}',
  'description:"Todos los flujos del espacio de trabajo se ejecutaron bien."}',
  "hito: workspace",
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
  <path d="M21.2 21.5H104.8A13.2 13.2 0 0 1 104.8 47.9H21.2A13.2 13.2 0 0 1 21.2 21.5ZM21.2 57.8H76.2A13.2 13.2 0 0 1 76.2 84.2H21.2A13.2 13.2 0 0 1 21.2 57.8ZM21.2 94.1H47.6A13.2 13.2 0 0 1 47.6 120.5H21.2A13.2 13.2 0 0 1 21.2 94.1ZM91.6 107.3a13.2 13.2 0 1 0 26.4 0a13.2 13.2 0 1 0 -26.4 0Z" fill="currentColor"/>
  <text x="142" y="92" fill="currentColor" font-family="-apple-system, BlinkMacSystemFont, 'SF Pro Display', Inter, sans-serif" font-size="68" font-weight="800" letter-spacing="-1.5">Clean Studio</text>
</svg>`;
fs.writeFileSync(path.join(OUT, "assets", "Maestro logo _ wordmark-CXPWZs15.svg"), logo);
// Icono de la app dentro de la interfaz (Acerca de, favicon): el mismo de src-tauri/icons
for (const name of fs.readdirSync(path.join(OUT, "assets"))) {
  if (/^maestro-icon-.*\.png$/.test(name)) {
    fs.copyFileSync(path.join(ROOT, "src-tauri", "icons", "icon.png"), path.join(OUT, "assets", name));
  }
}

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
// Ventana "Claude en Clean Studio" (página propia, fuera del bundle de la interfaz)
for (const name of ["claude.html", "claude.js"]) {
  fs.copyFileSync(path.join(ROOT, "src", "claude", name), path.join(OUT, name));
}
// El shim lleva incrustada la tabla de traducciones (la misma que usa errors.rs en Rust)
const errores = JSON.parse(fs.readFileSync(path.join(ROOT, "src", "shim", "errores_es.json"), "utf8"));
const shim = fs.readFileSync(path.join(ROOT, "src", "shim", "electron-shim.js"), "utf8");
fs.writeFileSync(
  path.join(OUT, "electron-shim.js"),
  `globalThis.CLEAN_STUDIO_ERRORES = ${JSON.stringify({ pasos: errores.pasos, errores: errores.errores })};\n${shim}`,
);

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
