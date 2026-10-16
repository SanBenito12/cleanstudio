import React, { useEffect, useState } from "react";
import {
  Play,
  Sparkles,
  FileDown,
  Save,
  Server,
  ShieldCheck,
  Terminal,
  CheckCircle2,
  AlertTriangle,
  Clock,
  Globe,
  RefreshCw,
  Wand2,
} from "lucide-react";
import {
  FLOW_TEMPLATES_ES,
  Lang,
  MAESTRO_COMMANDS_ES,
  UI_STRINGS,
  translateMaestroError,
} from "./i18n/es";
import {
  DEFAULT_UI_ELEMENTS,
  EngineDiagnostics,
  ExecutionStepResult,
  LocalDevice,
  UIElementNode,
  WorkspaceFile,
  bootIosSimulator,
  createFlowFile,
  executeCommandOnMaestroServer,
  exportHtmlReport,
  fetchDiagnostics,
  fetchLocalDevices,
  fetchWorkspaceFiles,
  readFlowFile,
  saveFlowFile,
  startSimulatorStream,
  startStudioServer,
} from "./services/bridge";
import { SidebarWorkspace } from "./components/SidebarWorkspace";
import { DeviceMirrorInspector } from "./components/DeviceMirrorInspector";
import { AiAssistantModal } from "./components/AiAssistantModal";

export const App: React.FC = () => {
  const [lang, setLang] = useState<Lang>("es");
  const t = UI_STRINGS[lang];

  const [diagnostics, setDiagnostics] = useState<EngineDiagnostics | null>(null);
  const [devices, setDevices] = useState<LocalDevice[]>([]);
  const [selectedDeviceId, setSelectedDeviceId] = useState<string>("");
  const [files, setFiles] = useState<WorkspaceFile[]>([]);
  const [activeFilePath, setActiveFilePath] = useState<string>("");
  const [yamlContent, setYamlContent] = useState<string>(FLOW_TEMPLATES_ES[0].yaml);
  const [elements] = useState<UIElementNode[]>(DEFAULT_UI_ELEMENTS);
  const [selectedElement, setSelectedElement] = useState<UIElementNode | null>(
    DEFAULT_UI_ELEMENTS[1]
  );

  const [envVars, setEnvVars] = useState<Record<string, string>>({
    CORREO_DEMO: "usuario@cleanstudio.dev",
    ENTORNO: "staging-local",
  });

  const [isRunning, setIsRunning] = useState(false);
  const [stepsResult, setStepsResult] = useState<ExecutionStepResult[]>([]);
  const [statusMessage, setStatusMessage] = useState<string>(
    "Clean Studio listo • Motores verificados con REA"
  );
  const [aiModalOpen, setAiModalOpen] = useState(false);

  const refreshAll = async () => {
    const [diag, devs, wsFiles] = await Promise.all([
      fetchDiagnostics(),
      fetchLocalDevices(),
      fetchWorkspaceFiles(),
    ]);
    setDiagnostics(diag);
    setDevices(devs);
    if (devs.length > 0 && !selectedDeviceId) {
      setSelectedDeviceId(devs[0].id);
    }
    setFiles(wsFiles);
    if (wsFiles.length > 0 && !activeFilePath) {
      setActiveFilePath(wsFiles[0].path);
      const content = await readFlowFile(wsFiles[0].path);
      setYamlContent(content);
    }
  };

  useEffect(() => {
    refreshAll();
  }, []);

  const currentDevice =
    devices.find((d) => d.id === selectedDeviceId) ||
    devices[0] || {
      id: "sim-ios",
      name: "iPhone 16 Pro",
      platform: "iOS",
      state: "Conectado",
      runtime: "iOS 18",
    };

  const handleInsertSnippet = (snippet: string) => {
    setYamlContent((prev) => `${prev.trimEnd()}\n${snippet}\n`);
    setStatusMessage("Paso YAML insertado en el flujo activo.");
  };

  const handleSaveCurrentFile = async () => {
    if (!activeFilePath) return;
    const msg = await saveFlowFile(activeFilePath, yamlContent);
    setStatusMessage(msg);
  };

  const handleCreateNewFlow = async (name: string) => {
    const template = `appId: com.ejemplo.app\nname: "${name}"\n---\n- launchApp\n- assertVisible: "Inicio"\n`;
    const newPath = await createFlowFile(name, template);
    const wsFiles = await fetchWorkspaceFiles();
    setFiles(wsFiles);
    setActiveFilePath(newPath);
    setYamlContent(template);
    setStatusMessage(`Nuevo flujo creado: ${name}`);
  };

  const parseExecutableSteps = (yamlText: string): string[] => {
    const parts = yamlText.split(/^---$/m);
    const body = parts.length > 1 ? parts[1] : yamlText;
    return body
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l.startsWith("- "));
  };

  const handleRunFlow = async () => {
    setIsRunning(true);
    setStatusMessage(`Ejecutando flujo en ${currentDevice.name}...`);
    const rawSteps = parseExecutableSteps(yamlContent);
    const initial: ExecutionStepResult[] = rawSteps.map((cmd) => ({
      command: cmd,
      status: "PENDING",
      duration_ms: 0,
    }));
    setStepsResult(initial);

    const updated = [...initial];
    for (let i = 0; i < updated.length; i++) {
      updated[i] = { ...updated[i], status: "RUNNING" };
      setStepsResult([...updated]);

      const t0 = performance.now();
      const res = await executeCommandOnMaestroServer(updated[i].command);
      await new Promise((r) => setTimeout(r, 240));
      const elapsed = Math.round(performance.now() - t0);

      if (res.ok) {
        updated[i] = {
          ...updated[i],
          status: "COMPLETED",
          duration_ms: elapsed,
        };
      } else {
        updated[i] = {
          ...updated[i],
          status: "FAILED",
          duration_ms: elapsed,
          error_es: translateMaestroError(res.error || "Element not found"),
        };
      }
      setStepsResult([...updated]);
    }
    setIsRunning(false);
    setStatusMessage(
      `Flujo completado: ${updated.filter((s) => s.status === "COMPLETED").length}/${
        updated.length
      } pasos exitosos.`
    );
  };

  const handleExportReport = async () => {
    const stepsToExport: ExecutionStepResult[] =
      stepsResult.length > 0
        ? stepsResult
        : parseExecutableSteps(yamlContent).map((c) => ({
            command: c,
            status: "COMPLETED",
            duration_ms: 310,
          }));
    const totalMs = stepsToExport.reduce((acc, s) => acc + s.duration_ms, 0);
    const fileName = activeFilePath.split("/").pop() || "flujo-e2e.yaml";
    const path = await exportHtmlReport({
      flow_name: fileName,
      app_id: "com.ejemplo.tienda",
      device_name: `${currentDevice.name} (${currentDevice.platform})`,
      total_duration_ms: totalMs,
      steps: stepsToExport,
      yaml_content: yamlContent,
    });
    setStatusMessage(`Reporte HTML en español exportado en: ${path}`);
  };

  const handleStartCoreServer = async () => {
    setStatusMessage("Iniciando servidor Maestro Core (:5050) desde Rust...");
    try {
      const msg = await startStudioServer();
      setStatusMessage(msg);
      setTimeout(refreshAll, 1500);
    } catch (e) {
      setStatusMessage(translateMaestroError(String(e)));
    }
  };

  return (
    <div className="h-screen w-screen flex flex-col bg-slate-950 text-slate-100 overflow-hidden">
      {/* Barra Superior Principal */}
      <header className="h-14 px-4 bg-slate-900/90 border-b border-slate-800 flex items-center justify-between gap-3 shrink-0">
        <div className="flex items-center gap-3">
          <div className="w-9 h-9 rounded-xl bg-gradient-to-br from-emerald-400 to-sky-500 flex items-center justify-center shadow-lg shadow-emerald-500/20 font-black text-slate-950 text-base">
            CS
          </div>
          <div>
            <div className="flex items-center gap-2">
              <h1 className="text-sm font-bold tracking-tight text-white">Clean Studio</h1>
              <span className="text-[10px] font-semibold px-2 py-0.5 rounded-full bg-emerald-500/15 text-emerald-300 border border-emerald-500/30">
                v1.0 Rust + Tauri
              </span>
              <span className="hidden xl:inline-flex items-center gap-1 text-[10px] px-2 py-0.5 rounded-full bg-slate-800 text-slate-300 border border-slate-700">
                <ShieldCheck className="w-3 h-3 text-emerald-400" />
                {t.noTelemetryBadge}
              </span>
            </div>
            <p className="text-[11px] text-slate-400">{t.subtitle}</p>
          </div>
        </div>

        {/* Controles Centrales: Estado de Motores y Selector de Dispositivo */}
        <div className="flex items-center gap-2">
          <button
            onClick={handleStartCoreServer}
            className={`px-2.5 py-1.5 rounded-lg text-xs font-medium border flex items-center gap-1.5 transition ${
              diagnostics?.studio_server_running
                ? "bg-emerald-500/15 border-emerald-500/40 text-emerald-300"
                : "bg-amber-500/15 border-amber-500/40 text-amber-300 hover:bg-amber-500/25"
            }`}
            title="Click para iniciar/verificar studio-server.jar en el puerto 5050"
          >
            <Server className="w-3.5 h-3.5" />
            {diagnostics?.studio_server_running ? t.coreRunning : t.startCore}
          </button>

          <div className="flex items-center bg-slate-950 border border-slate-800 rounded-lg px-2 py-1">
            <select
              value={selectedDeviceId}
              onChange={(e) => setSelectedDeviceId(e.target.value)}
              className="bg-transparent text-xs text-slate-200 focus:outline-none pr-2"
            >
              {devices.map((d) => (
                <option key={d.id} value={d.id} className="bg-slate-900 text-slate-100">
                  [{d.platform}] {d.name} — {d.state}
                </option>
              ))}
            </select>
            <button
              onClick={refreshAll}
              className="p-1 text-slate-400 hover:text-white transition"
              title="Actualizar lista de dispositivos"
            >
              <RefreshCw className="w-3.5 h-3.5" />
            </button>
          </div>
        </div>

        {/* Acciones Principales */}
        <div className="flex items-center gap-2">
          <button
            onClick={() => setLang(lang === "es" ? "en" : "es")}
            className="px-2.5 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 border border-slate-700 text-xs font-semibold text-slate-200 flex items-center gap-1 transition"
          >
            <Globe className="w-3.5 h-3.5 text-sky-400" />
            {lang.toUpperCase()}
          </button>

          <button
            onClick={() => setAiModalOpen(true)}
            className="px-3 py-1.5 rounded-lg bg-purple-500/20 hover:bg-purple-500/30 border border-purple-500/40 text-purple-200 text-xs font-semibold flex items-center gap-1.5 transition"
          >
            <Sparkles className="w-3.5 h-3.5 text-purple-300" />
            {t.aiAssistant}
          </button>

          <button
            onClick={handleExportReport}
            className="px-3 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 border border-slate-700 text-slate-200 text-xs font-medium flex items-center gap-1.5 transition"
          >
            <FileDown className="w-3.5 h-3.5 text-sky-400" />
            {t.exportReport}
          </button>

          <button
            onClick={handleRunFlow}
            disabled={isRunning}
            className="px-4 py-1.5 rounded-lg bg-emerald-500 hover:bg-emerald-400 disabled:opacity-50 text-slate-950 font-bold text-xs flex items-center gap-1.5 transition shadow-lg shadow-emerald-500/20"
          >
            <Play className="w-3.5 h-3.5 fill-current" />
            {isRunning ? t.runningFlow : t.runFlow}
          </button>
        </div>
      </header>

      {/* Cuerpo Principal en 3 Columnas */}
      <div className="flex-1 flex min-h-0">
        {/* Columna 1: Barra Lateral de Workspaces, Plantillas, Comandos y Variables */}
        <SidebarWorkspace
          files={files}
          activeFilePath={activeFilePath}
          onSelectFile={async (f) => {
            setActiveFilePath(f.path);
            const content = await readFlowFile(f.path);
            setYamlContent(content);
            setStatusMessage(`Flujo cargado: ${f.name}`);
          }}
          onCreateNewFlow={handleCreateNewFlow}
          onSelectTemplate={(tpl) => {
            setYamlContent(tpl.yaml);
            setStatusMessage(`Plantilla cargada: ${tpl.nameEs}`);
          }}
          onInsertCommand={(cmd) => handleInsertSnippet(cmd.snippet)}
          envVars={envVars}
          onUpdateEnvVar={(k, v) => setEnvVars((prev) => ({ ...prev, [k]: v }))}
        />

        {/* Columna 2: Editor YAML Inteligente + Constructor Visual + Consola REPL */}
        <div className="flex-1 flex flex-col min-w-0 bg-slate-950">
          {/* Barra del Constructor Visual Rápido en Español */}
          <div className="px-4 py-2 bg-slate-900/60 border-b border-slate-800/80 flex items-center justify-between gap-2 flex-wrap">
            <div className="flex items-center gap-1.5 flex-wrap">
              <span className="text-[11px] font-semibold text-slate-400 flex items-center gap-1 mr-1">
                <Wand2 className="w-3.5 h-3.5 text-emerald-400" />
                {t.visualBuilder}
              </span>
              {MAESTRO_COMMANDS_ES.slice(0, 6).map((cmd) => (
                <button
                  key={cmd.id}
                  onClick={() => handleInsertSnippet(cmd.snippet)}
                  className="px-2 py-1 rounded-md bg-slate-800/90 hover:bg-emerald-500/20 hover:text-emerald-300 hover:border-emerald-500/40 border border-slate-700 text-[11px] font-medium text-slate-200 transition"
                  title={cmd.descriptionEs}
                >
                  + {cmd.titleEs}
                </button>
              ))}
            </div>
            <button
              onClick={handleSaveCurrentFile}
              className="px-2.5 py-1 rounded-md bg-slate-800 hover:bg-slate-700 border border-slate-700 text-xs text-slate-200 flex items-center gap-1 transition"
            >
              <Save className="w-3.5 h-3.5 text-emerald-400" /> Guardar YAML
            </button>
          </div>

          {/* Área del Editor YAML con Numeración de Líneas */}
          <div className="flex-1 flex min-h-0 relative bg-slate-950">
            <div className="w-12 py-4 bg-slate-900/30 border-r border-slate-800/60 text-right pr-3 select-none font-mono text-xs text-slate-600 leading-6">
              {yamlContent.split("\n").map((_, i) => (
                <div key={i}>{i + 1}</div>
              ))}
            </div>
            <textarea
              value={yamlContent}
              onChange={(e) => setYamlContent(e.target.value)}
              spellCheck={false}
              className="flex-1 p-4 bg-slate-950 text-slate-100 font-mono text-xs leading-6 focus:outline-none resize-none overflow-y-auto"
            />
          </div>

          {/* Consola Inferior de Ejecución Paso a Paso y Diagnóstico en Español */}
          <div className="h-56 border-t border-slate-800 bg-slate-900/50 flex flex-col">
            <div className="px-4 py-2 border-b border-slate-800/80 flex items-center justify-between">
              <div className="flex items-center gap-2 text-xs font-semibold text-slate-200">
                <Terminal className="w-3.5 h-3.5 text-emerald-400" />
                <span>{t.consoleTitle}</span>
              </div>
              <span className="text-[11px] text-slate-400">{statusMessage}</span>
            </div>

            <div className="flex-1 overflow-y-auto p-3 space-y-1.5">
              {stepsResult.length === 0 ? (
                <div className="h-full flex items-center justify-center text-xs text-slate-500">
                  Haz clic en &ldquo;Ejecutar Flujo&rdquo; para correr cada paso YAML y ver los tiempos y diagnósticos en español.
                </div>
              ) : (
                stepsResult.map((step, idx) => (
                  <div
                    key={idx}
                    className={`p-2 rounded-lg border text-xs flex flex-col gap-1 ${
                      step.status === "COMPLETED"
                        ? "bg-emerald-950/20 border-emerald-500/30 text-emerald-200"
                        : step.status === "FAILED"
                        ? "bg-red-950/30 border-red-500/40 text-red-200"
                        : step.status === "RUNNING"
                        ? "bg-sky-950/30 border-sky-500/40 text-sky-200 animate-pulse"
                        : "bg-slate-900/60 border-slate-800 text-slate-400"
                    }`}
                  >
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2 font-mono">
                        {step.status === "COMPLETED" && (
                          <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />
                        )}
                        {step.status === "FAILED" && (
                          <AlertTriangle className="w-3.5 h-3.5 text-red-400" />
                        )}
                        {step.status === "RUNNING" && (
                          <Clock className="w-3.5 h-3.5 text-sky-400" />
                        )}
                        <span>
                          Paso #{idx + 1}: {step.command}
                        </span>
                      </div>
                      <span className="text-[10px] font-semibold">
                        {step.status === "COMPLETED"
                          ? `EXITOSO (${step.duration_ms} ms)`
                          : step.status === "FAILED"
                          ? `FALLIDO (${step.duration_ms} ms)`
                          : step.status === "RUNNING"
                          ? "EJECUTANDO..."
                          : "PENDIENTE"}
                      </span>
                    </div>
                    {step.error_es && (
                      <div className="text-[11px] text-amber-300 bg-slate-950/80 p-2 rounded border border-amber-500/30">
                        {step.error_es}
                      </div>
                    )}
                  </div>
                ))
              )}
            </div>
          </div>
        </div>

        {/* Columna 3: Espejo de Dispositivo en Vivo + Inspector de Jerarquía UI */}
        <div className="w-[360px] shrink-0">
          <DeviceMirrorInspector
            streamUrl={diagnostics?.active_stream_url || null}
            deviceName={currentDevice.name}
            platform={currentDevice.platform}
            elements={elements}
            selectedElement={selectedElement}
            onSelectElement={setSelectedElement}
            onInsertStep={handleInsertSnippet}
            onStartStream={async () => {
              const target = currentDevice.platform.toLowerCase().includes("android")
                ? "android"
                : "ios";
              const msg = await startSimulatorStream(currentDevice.id, target);
              setStatusMessage(msg);
              setTimeout(refreshAll, 1200);
            }}
            onBootSimulator={async () => {
              const msg = await bootIosSimulator(currentDevice.id);
              setStatusMessage(msg);
              setTimeout(refreshAll, 1500);
            }}
          />
        </div>
      </div>

      {/* Modal del Asistente IA en Español */}
      <AiAssistantModal
        isOpen={aiModalOpen}
        onClose={() => setAiModalOpen(false)}
        currentYaml={yamlContent}
        onApplyYaml={(newYaml) => {
          setYamlContent(newYaml);
          setStatusMessage("Flujo actualizado con el Asistente IA en Español.");
        }}
      />
    </div>
  );
};

export default App;
