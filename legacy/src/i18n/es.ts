export type Lang = "es" | "en";

export interface MaestroCommandDoc {
  id: string;
  name: string;
  titleEs: string;
  titleEn: string;
  category: "interaccion" | "verificacion" | "navegacion" | "sistema";
  descriptionEs: string;
  snippet: string;
}

export interface FlowTemplate {
  id: string;
  nameEs: string;
  descriptionEs: string;
  category: string;
  yaml: string;
}

export const MAESTRO_COMMANDS_ES: MaestroCommandDoc[] = [
  {
    id: "tapOn",
    name: "tapOn",
    titleEs: "Tocar elemento",
    titleEn: "Tap on element",
    category: "interaccion",
    descriptionEs: "Toca un botón, texto o ID en la pantalla del dispositivo.",
    snippet: `- tapOn:\n    text: "Continuar"`,
  },
  {
    id: "inputText",
    name: "inputText",
    titleEs: "Escribir texto",
    titleEn: "Input text",
    category: "interaccion",
    descriptionEs: "Escribe texto en el campo que tiene el foco actualmente.",
    snippet: `- inputText: "usuario@cleanstudio.dev"`,
  },
  {
    id: "assertVisible",
    name: "assertVisible",
    titleEs: "Verificar visible",
    titleEn: "Assert visible",
    category: "verificacion",
    descriptionEs: "Comprueba que un elemento o texto esté visible en pantalla.",
    snippet: `- assertVisible: "Bienvenido"`,
  },
  {
    id: "assertNotVisible",
    name: "assertNotVisible",
    titleEs: "Verificar oculto",
    titleEn: "Assert not visible",
    category: "verificacion",
    descriptionEs: "Comprueba que un elemento o mensaje de error ya no esté visible.",
    snippet: `- assertNotVisible: "Cargando..."`,
  },
  {
    id: "scrollUntilVisible",
    name: "scrollUntilVisible",
    titleEs: "Deslizar hasta ver",
    titleEn: "Scroll until visible",
    category: "navegacion",
    descriptionEs: "Desliza automáticamente la pantalla hasta encontrar el elemento indicado.",
    snippet: `- scrollUntilVisible:\n    element: "Comprar ahora"\n    direction: DOWN`,
  },
  {
    id: "swipe",
    name: "swipe",
    titleEs: "Deslizar (Swipe)",
    titleEn: "Swipe",
    category: "navegacion",
    descriptionEs: "Desliza en una dirección (LEFT, RIGHT, UP, DOWN).",
    snippet: `- swipe:\n    direction: LEFT`,
  },
  {
    id: "launchApp",
    name: "launchApp",
    titleEs: "Iniciar aplicación",
    titleEn: "Launch app",
    category: "sistema",
    descriptionEs: "Abre la aplicación objetivo, opcionalmente limpiando sus datos previos.",
    snippet: `- launchApp:\n    clearState: true`,
  },
  {
    id: "extendedWaitUntil",
    name: "extendedWaitUntil",
    titleEs: "Esperar condición",
    titleEn: "Extended wait until",
    category: "verificacion",
    descriptionEs: "Espera un tiempo máximo (en ms) hasta que aparezca un elemento.",
    snippet: `- extendedWaitUntil:\n    visible: "Panel Principal"\n    timeout: 10000`,
  },
  {
    id: "takeScreenshot",
    name: "takeScreenshot",
    titleEs: "Capturar pantalla",
    titleEn: "Take screenshot",
    category: "sistema",
    descriptionEs: "Guarda una captura de pantalla PNG como evidencia del flujo.",
    snippet: `- takeScreenshot: "evidencia-paso"`,
  },
  {
    id: "eraseText",
    name: "eraseText",
    titleEs: "Borrar texto",
    titleEn: "Erase text",
    category: "interaccion",
    descriptionEs: "Borra el número indicado de caracteres en el campo activo.",
    snippet: `- eraseText: 20`,
  },
  {
    id: "back",
    name: "back",
    titleEs: "Botón Atrás",
    titleEn: "Navigate Back",
    category: "navegacion",
    descriptionEs: "Regresa a la pantalla anterior del sistema.",
    snippet: `- back`,
  },
  {
    id: "openLink",
    name: "openLink",
    titleEs: "Abrir Deep Link / URL",
    titleEn: "Open Link",
    category: "sistema",
    descriptionEs: "Abre una URL web o un esquema deep-link directamente en el dispositivo.",
    snippet: `- openLink: "miapp://checkout"`,
  },
];

export const FLOW_TEMPLATES_ES: FlowTemplate[] = [
  {
    id: "login-ecommerce",
    nameEs: "Inicio de Sesión + Carrito E-Commerce",
    descriptionEs: "Flujo completo con variables de entorno, login, scroll automático y captura de evidencia.",
    category: "Móvil (Android / iOS)",
    yaml: `appId: com.ejemplo.tienda
name: "Inicio de Sesión y Carrito de Compras"
tags:
  - humo
  - e2e
env:
  CORREO_DEMO: "qa@cleanstudio.dev"
---
# 1. Abrir la app desde cero
- launchApp:
    clearState: true

# 2. Autenticación de usuario
- assertVisible: "Iniciar sesión"
- tapOn:
    id: "input_correo"
- inputText: \${CORREO_DEMO}
- tapOn:
    id: "input_password"
- inputText: "SuperClave2026!"
- tapOn: "Entrar"

# 3. Buscar producto y agregar al carrito
- extendedWaitUntil:
    visible: "Catálogo Principal"
    timeout: 8000
- scrollUntilVisible:
    element: "Agregar al carrito"
    direction: DOWN
- tapOn: "Agregar al carrito"
- assertVisible: "1 artículo en tu carrito"
- takeScreenshot: "carrito-validado"`,
  },
  {
    id: "onboarding-permisos",
    nameEs: "Onboarding + Permisos del Sistema",
    descriptionEs: "Recorre carrusel de bienvenida otorgando permisos de notificaciones y ubicación.",
    category: "Móvil (Android / iOS)",
    yaml: `appId: com.ejemplo.appmovil
name: "Onboarding y Permisos Nativos"
---
- launchApp:
    clearState: true
    permissions:
      notifications: allow
      location: allow
- swipe:
    direction: LEFT
- swipe:
    direction: LEFT
- tapOn: "Comenzar ahora"
- assertVisible: "Mi Cuenta"`,
  },
  {
    id: "web-e2e",
    nameEs: "Automatización Web E2E",
    descriptionEs: "Prueba un sitio web de principio a fin usando el motor Web de Maestro.",
    category: "Web Browser",
    yaml: `url: https://example.com
name: "Flujo Web E2E — Validación de Portada"
---
- launchApp
- assertVisible: "Example Domain"
- tapOn: "More information..."
- takeScreenshot: "web-verificada"`,
  },
  {
    id: "formulario-validacion",
    nameEs: "Validación de Errores en Formulario",
    descriptionEs: "Verifica mensajes de alerta cuando se envían campos vacíos o inválidos.",
    category: "QA / Regresión",
    yaml: `appId: com.ejemplo.app
name: "Validación de Campos Requeridos"
---
- launchApp
- tapOn: "Crear cuenta"
- tapOn: "Guardar registro"
- assertVisible: "El correo electrónico es obligatorio"
- tapOn:
    id: "campo_email"
- inputText: "correo-invalido"
- assertVisible: "Ingresa un correo válido"`,
  },
];

export function translateMaestroError(rawError: string): string {
  const lower = rawError.toLowerCase();
  if (lower.includes("element not found") || lower.includes("unable to find element")) {
    return "No se encontró el elemento en la pantalla actual. Sugerencia: Usa el Inspector Visual a la derecha para verificar el ID o texto exacto, o agrega un paso 'extendedWaitUntil' antes de tocarlo.";
  }
  if (lower.includes("deviceunreachable") || lower.includes("no devices") || lower.includes("disconnected")) {
    return "El dispositivo o simulador no responde. Verifica que el simulador iOS esté encendido ('Booted') o que el dispositivo Android aparezca en 'adb devices'.";
  }
  if (lower.includes("appinstallexception") || lower.includes("unable to launch app")) {
    return "No se pudo abrir el 'appId' configurado. Comprueba que la aplicación esté instalada en el simulador o que el 'appId' en la cabecera del YAML sea el correcto.";
  }
  if (lower.includes("port 5050") || lower.includes("connection refused")) {
    return "El motor local Maestro Core (:5050) está iniciando o detenido. Haz clic en el botón 'Iniciar Motor Core' en la barra superior.";
  }
  if (lower.includes("yaml") || lower.includes("syntax") || lower.includes("parse")) {
    return "Error de sintaxis en el archivo YAML. Revisa la indentación (usa 2 espacios por nivel) y que la línea '---' separe la cabecera de la lista de comandos.";
  }
  return `Detalle técnico traducido: ${rawError}`;
}

export const UI_STRINGS = {
  es: {
    subtitle: "Automatización E2E Móvil y Web • Núcleo Rust + Maestro Core • Cero Telemetría",
    coreRunning: "Motor Core (:5050) Activo",
    coreStopped: "Motor Core (:5050) Detenido",
    streamActive: "Stream Rust 60 FPS",
    streamIdle: "Stream Listo",
    startCore: "Iniciar Motor",
    aiAssistant: "Asistente IA",
    exportReport: "Exportar Reporte HTML",
    runFlow: "Ejecutar Flujo",
    runningFlow: "Ejecutando...",
    tabFlows: "Flujos",
    tabTemplates: "Plantillas",
    tabCommands: "Comandos",
    tabEnv: "Variables",
    visualBuilder: "Constructor Visual Rápido:",
    inspectorTitle: "Espejo en Vivo e Inspector de Jerarquía",
    hierarchyTitle: "Árbol de Elementos UI (Haz clic para insertar en YAML)",
    consoleTitle: "Ejecución Paso a Paso y Consola Traducida al Español",
    noTelemetryBadge: "100% Local • Sin Telemetría • Sin Cuenta Cloud",
  },
  en: {
    subtitle: "Mobile & Web E2E Automation • Rust Core + Maestro Engine • Zero Telemetry",
    coreRunning: "Core Engine (:5050) Online",
    coreStopped: "Core Engine (:5050) Offline",
    streamActive: "Rust Stream 60 FPS",
    streamIdle: "Stream Ready",
    startCore: "Start Engine",
    aiAssistant: "AI Assistant",
    exportReport: "Export HTML Report",
    runFlow: "Run Flow",
    runningFlow: "Running...",
    tabFlows: "Flows",
    tabTemplates: "Templates",
    tabCommands: "Commands",
    tabEnv: "Env Vars",
    visualBuilder: "Quick Visual Step Builder:",
    inspectorTitle: "Live Device Mirror & Hierarchy Inspector",
    hierarchyTitle: "UI Element Tree (Click to insert into YAML)",
    consoleTitle: "Step-by-Step Execution & Diagnostic Console",
    noTelemetryBadge: "100% Local • Zero Telemetry • No Cloud Paywall",
  },
};
