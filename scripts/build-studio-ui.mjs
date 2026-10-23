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
  // Cabecera: solo el nombre de la carpeta; la ruta completa queda en el title
  [
    'k("span",{className:"text-sm font-medium truncate",title:v,children:v})',
    'k("span",{className:"text-sm font-medium truncate",title:v,children:a?hn.basename(a):v})',
    "cabecera: nombre de la carpeta",
  ],
  // "Acerca de": la fila del motor
  [
    'k("span",{className:"text-sm text-muted-foreground",children:"Maestro"}),qO?',
    'k("span",{className:"text-sm text-muted-foreground",children:"Motor"}),qO?',
    "acerca de: motor",
  ],
];
for (const [from, to, label] of UI_PATCHES) js = mustReplace(js, from, to, label);

// Cabecera: botón "Claude" junto al selector de tema (abre "Conectar con Claude…" del backend),
// con el icono original de la app de Claude (src/claude/claude.png, copiado junto a index.html)
const claudeButton =
  'k(xt,{variant:"ghost",size:"sm",className:"gap-1.5 px-2.5 font-medium",' +
  'title:"Conectar con Claude: deja que Claude vea tus dispositivos, escriba y ejecute pruebas",' +
  '"aria-label":"Conectar con Claude",onClick:()=>ur("cs-connect-claude"),children:[' +
  'k("img",{src:"./claude.png",width:16,height:16,alt:"","aria-hidden":!0,className:"rounded-sm"}),' +
  'k("span",{children:"Claude"})]})';
js = mustReplace(
  js,
  'children:[k(NBe,{}),e&&k(xt,{variant:"ghost",size:"icon-sm",onClick:e,className:"text-amber-500 hover:text-amber-400"',
  `children:[${claudeButton},k(NBe,{}),e&&k(xt,{variant:"ghost",size:"icon-sm",onClick:e,className:"text-amber-500 hover:text-amber-400"`,
  "cabecera: botón Claude",
);
// Panel del dispositivo vacío: en vez de "Ningún dispositivo conectado", la lista de simuladores y
// emuladores para encender o conectar con un clic (misma acción que el selector, triggerActivateDevice).
// Usa los helpers del selector OWe: vLe (nombre/SO), hYt (relojes fuera), gYt (icono de plataforma).
// El CSS de Tailwind viene compilado: solo sirven clases que ya existen en studio-ui/assets/*.css.
const emptyDevicePanel = `function CsSinDispositivo(){const o=d=>d.device.instanceId?0:{IOS:1,ANDROID:1,IPADOS:2}[fYt(d)]??3;
const{devices:s,isDevicesLoading:c,triggerActivateDevice:l}=Xf(),[t,v]=D.useState(!1),[p,q]=D.useState(null),
L=D.useMemo(()=>(s??[]).map(d=>({device:d,parsed:vLe(d)})).filter(d=>!hYt(d)).sort((a,b)=>o(a)-o(b)||a.parsed.name.localeCompare(b.parsed.name)),[s]),
V=t?L:L.slice(0,5),
F=(h,b)=>X("div",{className:"text-center flex flex-col items-center gap-1.5",children:[
k("svg",{viewBox:"0 0 24 24",width:28,height:28,fill:"none",stroke:"currentColor",strokeWidth:1.5,strokeLinecap:"round",className:"text-muted-foreground mb-1","aria-hidden":!0,children:[k("rect",{x:6,y:2.5,width:12,height:19,rx:2.5}),k("path",{d:"M10.5 18.5h3"})]}),
k("p",{className:"text-sm font-medium text-foreground",children:h}),
k("p",{className:"text-xs text-muted-foreground max-w-64 text-balance",children:b})]});
if(!L.length)return k("div",{className:"px-6",children:c||!s?F("Buscando dispositivos…","Un momento, revisando los simuladores y emuladores de este Mac."):F("No hay simuladores instalados","Crea uno en Xcode (Settings › Components) o en el Device Manager de Android Studio. Aparecerá aquí solo.")});
return X("div",{className:"w-full max-h-full overflow-y-auto px-4 py-8 flex flex-col gap-4",onClick:e=>e.stopPropagation(),onKeyDown:e=>e.stopPropagation(),children:[
F("Elige un dispositivo","Enciéndelo aquí para ver su pantalla y ejecutar tus flujos."),
X("div",{className:"flex flex-col gap-1",children:[V.map(({device:d,parsed:n})=>{const i=d.instanceId??d.modelId,r=!!d.instanceId,w=p===i;
return X("button",{type:"button",disabled:p!==null,onClick:async()=>{q(i);try{await l(d)}finally{q(null)}},
className:"w-full flex items-center gap-3 rounded-xl px-3 py-2 text-left hover:bg-accent disabled:opacity-50 outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50 transition-colors",children:[
k("span",{className:"text-muted-foreground",children:gYt(d.platform)}),
X("span",{className:"min-w-0 flex-1",children:[k("span",{className:"block truncate text-sm text-foreground",children:n.name}),
X("span",{className:"flex items-center gap-1.5 text-xs text-muted-foreground",children:[r&&k("span",{className:"inline-block size-2 rounded-full bg-emerald-500","aria-hidden":"true"}),k("span",{className:"truncate",children:(n.os??bLe(d.platform))+(r?" · encendido":"")})]})]}),
k("span",{className:"text-xs font-medium text-muted-foreground whitespace-nowrap",children:w?(r?"Conectando…":"Encendiendo…"):r?"Conectar":"Encender"})]},i)}),
L.length>5&&k("button",{type:"button",onClick:()=>v(!t),className:"mx-auto mt-1 rounded-md px-2 py-1 text-xs text-muted-foreground hover:text-foreground underline-offset-4 hover:underline outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50",children:t?"Ver menos":\`Ver los \${L.length} dispositivos\`})]})]})}`.replace(/\n/g, "");
js = mustReplace(js, "function OWe(", `${emptyDevicePanel}function OWe(`, "panel vacío: componente");
js = mustReplace(
  js,
  'at==="idle"&&k("span",{className:"text-muted-foreground",children:"Ningún dispositivo conectado"})',
  'at==="idle"&&k(CsSinDispositivo,{})',
  "panel vacío: uso",
);
// Árbol de archivos: punto con el último resultado de cada flujo (verde pasó, rojo falló, ámbar
// incompleta). Al tocarlo se abre su reporte. Los datos vienen de runs.rs (cs-estados-flujos), que
// también los reenvía al terminar una ejecución o al abrir otro workspace. Mientras no haya datos, cada
// fila nueva vuelve a pedirlos: la primera petición puede llegar antes de que exista el workspace.
const flowStatusDot = `const CsEstados={m:{},l:new Set,c:!1,
fijar(m){this.m=m||{},this.l.forEach(f=>f())},
cargar(){clearTimeout(this.t),this.t=setTimeout(()=>_r("cs-estados-flujos").then(m=>this.fijar(m)).catch(()=>{}),200)},
sub(f){this.c||(this.c=!0,Ja("cs-estados-flujos",m=>this.fijar(m))),Object.keys(this.m).length||this.cargar();this.l.add(f);return()=>this.l.delete(f)}};
function CsEstadoFlujo({path:p}){
const m=D.useSyncExternalStore(f=>CsEstados.sub(f),()=>CsEstados.m),e=p?m[p]:null;if(!e)return null;
const ok=e.estado==="COMPLETED",mal=e.estado==="FAILED",n=ok?"Pasó":mal?"Falló":"Incompleta",
s=e.fin?Math.round((Date.parse(e.fin)-Date.now())/6e4):null,
r=s===null||isNaN(s)?"":" "+(Math.abs(s)<60?new Intl.RelativeTimeFormat("es",{numeric:"auto"}).format(s,"minute"):Math.abs(s)<2880?new Intl.RelativeTimeFormat("es",{numeric:"auto"}).format(Math.round(s/60),"hour"):new Intl.RelativeTimeFormat("es",{numeric:"auto"}).format(Math.round(s/1440),"day")),
t=n+r+(e.reporte?". Abrir reporte":"");
return k("button",{type:"button",title:t,"aria-label":t,disabled:!e.reporte,
onClick:x=>{x.stopPropagation(),_r("cs-abrir-reporte",e.reporte).catch(y=>Bn.error(String(y&&y.message||y)))},
onMouseDown:x=>x.stopPropagation(),
className:"ml-auto mr-1 shrink-0 inline-flex items-center justify-center size-4 rounded-full cursor-pointer hover:bg-accent outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50",
children:k("span",{className:"size-2 rounded-full "+(ok?"bg-emerald-500":mal?"bg-red-500":""),style:ok||mal?void 0:{background:"#f59e0b"}})})}`.replace(/\n/g, "");
js = mustReplace(js, "function OWe(", `${flowStatusDot}function OWe(`, "estado de flujos: componente");
// Barra de ejecución (fln): junto a "Ejecuciones", por qué no se puede ejecutar o el último resultado
// del flujo abierto. p = pestaña activa, E = hay dispositivo, Ne/de = flujo/workspace en curso.
// "Revisar con Claude" (revision.rs): Claude mira las capturas de esa ejecución sin abrir ventana;
// al terminar llega cs-revision-claude y el aviso abre el reporte actualizado.
// Sin dispositivo se avisa igual, pero la revisión de la última ejecución no lo necesita.
const runSummary = `function CsResumenEjecucion({path:p,device:d,yaml:y,running:r}){
const m=D.useSyncExternalStore(f=>CsEstados.sub(f),()=>CsEstados.m),[o,s]=D.useState(!1);
D.useEffect(()=>Ja("cs-revision-claude",x=>{s(!1);const v={action:{label:"Ver reporte",onClick:()=>_r("cs-abrir-reporte",x.reporte).catch(()=>{})}};x.ok?Bn.success(x.mensaje,v):Bn.error(x.mensaje)}),[]);
if(r)return null;
const c="truncate text-xs text-muted-foreground",e=p?m[p]:null;
const ok=e&&e.estado==="COMPLETED",mal=e&&e.estado==="FAILED",t="Última vez: "+(ok?"pasó":mal?"falló":"incompleta");
return X("div",{className:"flex min-w-0 items-center gap-1",children:[!d?k("span",{className:c,title:"Conecta un dispositivo para ejecutar pruebas",children:"Sin dispositivo"}):!y?k("span",{className:c,title:"Abre un flujo .yaml para ejecutarlo",children:"Abre un flujo"}):e&&X("button",{type:"button",disabled:!e.reporte,title:e.reporte?"Abrir reporte":void 0,onClick:()=>_r("cs-abrir-reporte",e.reporte).catch(x=>Bn.error(String(x&&x.message||x))),
className:"flex min-w-0 items-center gap-1.5 rounded-sm px-1 text-xs text-muted-foreground hover:text-foreground hover:underline underline-offset-4 cursor-pointer outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50",
children:[k("span",{className:"size-2 shrink-0 rounded-full "+(ok?"bg-emerald-500":mal?"bg-red-500":""),style:ok||mal?void 0:{background:"#f59e0b"}}),k("span",{className:"truncate",children:t})]}),
e&&e.reporte&&X("button",{type:"button",disabled:o,title:"Revisar con Claude: mira las pantallas de esta ejecución y anota lo que se vea mal",
onClick:()=>{s(!0),_r("cs-revisar-con-claude",e.reporte).then(x=>{x==="desktop"?(s(!1),Bn.success("Copié el pedido. Pégalo en un chat nuevo de Claude.")):Bn.success("Claude está revisando las pantallas. Te aviso cuando termine.")}).catch(x=>{s(!1),Bn.error(String(x&&x.message||x))})},
className:"flex shrink-0 items-center gap-1 rounded-sm px-1.5 py-0.5 text-xs font-medium text-muted-foreground hover:text-foreground hover:bg-accent disabled:opacity-50 cursor-pointer outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50",
"aria-label":"Revisar con Claude",children:[k("img",{src:"./claude.png",width:12,height:12,alt:"","aria-hidden":!0,className:"rounded-sm"}),o?"Revisando…":"Revisar"]})]})}`.replace(/\n/g, "");
js = mustReplace(js, "function OWe(", `${runSummary}function OWe(`, "barra de ejecución: componente");
// Paso fallido (u() dentro de GIe; m = ruta del flujo, w = paso): "Arreglar con Claude" abre Claude
// Code en el workspace con el error ya escrito, o copia el pedido y abre Claude Desktop (claude.rs).
const fixWithClaude = `function CsArreglarConClaude({archivo:a,paso:p,error:e}){
const[o,s]=D.useState(!1);
return X("button",{type:"button",disabled:o,onClick:x=>{x.stopPropagation(),s(!0),_r("cs-arreglar-con-claude",{archivo:a,paso:p,error:e}).then(m=>{m==="desktop"?Bn.success("Copié el pedido. Pégalo en un chat nuevo de Claude."):Bn.success("Claude Code se abrió en la Terminal con el error.")}).catch(y=>Bn.error(String(y&&y.message||y))).finally(()=>s(!1))},
className:"mt-1 inline-flex items-center gap-1.5 rounded-md border border-border px-2 py-0.5 text-xs font-medium text-foreground hover:bg-accent disabled:opacity-50 cursor-pointer outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50",
children:[k("img",{src:"./claude.png",width:14,height:14,alt:"","aria-hidden":!0,className:"rounded-sm"}),o?"Abriendo Claude…":"Arreglar con Claude"]})}`.replace(/\n/g, "");
js = mustReplace(js, "function OWe(", `${fixWithClaude}function OWe(`, "arreglar con Claude: componente");
js = mustReplace(
  js,
  'E&&w.errorMessage?k("p",{className:`mt-0.5 break-words ${S?"text-destructive":"text-orange-500"}`,children:w.errorMessage}):null,',
  'E&&w.errorMessage?k("p",{className:`mt-0.5 break-words ${S?"text-destructive":"text-orange-500"}`,children:w.errorMessage}):null,S&&m?k(CsArreglarConClaude,{archivo:m,paso:w.label,error:w.errorMessage??""}):null,',
  "arreglar con Claude: paso fallido",
);
js = mustReplace(
  js,
  'children:[k("span",{className:"text-sm font-medium text-foreground",children:"Ejecuciones"}),X("div",{className:"flex items-center gap-(--spacing-gap)"',
  'children:[X("div",{className:"flex min-w-0 overflow-hidden items-center gap-(--spacing-gap)",children:[k("span",{className:"text-sm font-medium text-foreground shrink-0",children:"Ejecuciones"}),k(CsResumenEjecucion,{path:p,device:E,yaml:!!p&&$k(p),running:Ne||de})]}),X("div",{className:"flex shrink-0 items-center gap-(--spacing-gap)"',
  "barra de ejecución: uso",
);
js = mustReplace(
  js,
  'children:X("span",{"data-drag-preview-content":!0,className:"flex items-center gap-(--spacing-gap-sm) text-sm",children:[k(Ge,{icon:lw,size:16,className:"text-muted-foreground"}),k("span",{className:"truncate",children:i.name})]})})',
  'children:[X("span",{"data-drag-preview-content":!0,className:"flex min-w-0 items-center gap-(--spacing-gap-sm) text-sm",children:[k(Ge,{icon:lw,size:16,className:"text-muted-foreground"}),k("span",{className:"truncate",children:i.name})]}),k(CsEstadoFlujo,{path:i.path})]})',
  "estado de flujos: árbol",
);
// Antes de conectar, el backend cierra el driver de Maestro en los otros simuladores de iOS
// (simulator.rs → liberar_driver_ios): comparten el puerto 22087 y el motor hablaría con el equivocado.
js = mustReplace(
  js,
  "async function xye(i){const e=await fetch(",
  'async function xye(i){await _r("cs-antes-de-conectar",i).catch(()=>{});const e=await fetch(',
  "conectar: liberar el driver de iOS",
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
for (const name of ["claude.html", "claude.js", "claude.png"]) {
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
