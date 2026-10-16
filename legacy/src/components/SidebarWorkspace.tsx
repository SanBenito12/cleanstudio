import React, { useState } from "react";
import {
  FolderOpen,
  FileCode2,
  Plus,
  BookOpen,
  Terminal,
  Sliders,
  Sparkles,
  CheckCircle2,
} from "lucide-react";
import {
  FLOW_TEMPLATES_ES,
  FlowTemplate,
  MAESTRO_COMMANDS_ES,
  MaestroCommandDoc,
} from "../i18n/es";
import { WorkspaceFile } from "../services/bridge";

interface Props {
  files: WorkspaceFile[];
  activeFilePath: string;
  onSelectFile: (file: WorkspaceFile) => void;
  onCreateNewFlow: (name: string) => void;
  onSelectTemplate: (tpl: FlowTemplate) => void;
  onInsertCommand: (cmd: MaestroCommandDoc) => void;
  envVars: Record<string, string>;
  onUpdateEnvVar: (key: string, val: string) => void;
}

export const SidebarWorkspace: React.FC<Props> = ({
  files,
  activeFilePath,
  onSelectFile,
  onCreateNewFlow,
  onSelectTemplate,
  onInsertCommand,
  envVars,
  onUpdateEnvVar,
}) => {
  const [activeTab, setActiveTab] = useState<"flujos" | "plantillas" | "comandos" | "variables">(
    "flujos"
  );
  const [newFileName, setNewFileName] = useState("");
  const [newEnvKey, setNewEnvKey] = useState("");
  const [newEnvVal, setNewEnvVal] = useState("");

  return (
    <div className="w-72 bg-slate-950 border-r border-slate-800/80 flex flex-col h-full">
      {/* Pestañas Superiores */}
      <div className="grid grid-cols-4 border-b border-slate-800/80 bg-slate-900/50 text-[11px] font-medium">
        <button
          onClick={() => setActiveTab("flujos")}
          className={`py-2.5 flex flex-col items-center gap-1 border-b-2 transition ${
            activeTab === "flujos"
              ? "border-emerald-400 text-emerald-300 bg-emerald-500/5"
              : "border-transparent text-slate-400 hover:text-slate-200"
          }`}
        >
          <FolderOpen className="w-3.5 h-3.5" />
          Flujos
        </button>
        <button
          onClick={() => setActiveTab("plantillas")}
          className={`py-2.5 flex flex-col items-center gap-1 border-b-2 transition ${
            activeTab === "plantillas"
              ? "border-emerald-400 text-emerald-300 bg-emerald-500/5"
              : "border-transparent text-slate-400 hover:text-slate-200"
          }`}
        >
          <BookOpen className="w-3.5 h-3.5" />
          Plantillas
        </button>
        <button
          onClick={() => setActiveTab("comandos")}
          className={`py-2.5 flex flex-col items-center gap-1 border-b-2 transition ${
            activeTab === "comandos"
              ? "border-emerald-400 text-emerald-300 bg-emerald-500/5"
              : "border-transparent text-slate-400 hover:text-slate-200"
          }`}
        >
          <Terminal className="w-3.5 h-3.5" />
          Comandos
        </button>
        <button
          onClick={() => setActiveTab("variables")}
          className={`py-2.5 flex flex-col items-center gap-1 border-b-2 transition ${
            activeTab === "variables"
              ? "border-emerald-400 text-emerald-300 bg-emerald-500/5"
              : "border-transparent text-slate-400 hover:text-slate-200"
          }`}
        >
          <Sliders className="w-3.5 h-3.5" />
          Entorno
        </button>
      </div>

      {/* Contenido de la Pestaña Activa */}
      <div className="flex-1 overflow-y-auto p-3 space-y-2">
        {activeTab === "flujos" && (
          <>
            <div className="flex gap-1.5 mb-3">
              <input
                type="text"
                value={newFileName}
                onChange={(e) => setNewFileName(e.target.value)}
                placeholder="nuevo-flujo.yaml"
                className="flex-1 px-2.5 py-1.5 rounded-lg bg-slate-900 border border-slate-800 text-xs text-slate-100 placeholder-slate-500 focus:outline-none focus:border-emerald-500"
              />
              <button
                onClick={() => {
                  if (newFileName.trim()) {
                    onCreateNewFlow(newFileName.trim());
                    setNewFileName("");
                  }
                }}
                className="px-2.5 py-1.5 rounded-lg bg-emerald-500/20 hover:bg-emerald-500/30 text-emerald-300 border border-emerald-500/40 text-xs font-medium flex items-center gap-1 transition"
                title="Crear nuevo flujo YAML"
              >
                <Plus className="w-3.5 h-3.5" /> Crear
              </button>
            </div>
            <div className="text-[10px] font-semibold uppercase tracking-wider text-slate-500 px-1 mb-1">
              Espacio de Trabajo (sample-flows)
            </div>
            {files.map((f) => {
              const isSelected = f.path === activeFilePath;
              return (
                <button
                  key={f.path}
                  onClick={() => onSelectFile(f)}
                  className={`w-full text-left p-2.5 rounded-xl border transition flex items-center gap-2.5 ${
                    isSelected
                      ? "bg-emerald-500/15 border-emerald-500/40 text-white shadow-sm"
                      : "bg-slate-900/40 border-slate-800/70 hover:bg-slate-900 text-slate-300"
                  }`}
                >
                  <FileCode2
                    className={`w-4 h-4 shrink-0 ${
                      isSelected ? "text-emerald-400" : "text-sky-400"
                    }`}
                  />
                  <div className="min-w-0 flex-1">
                    <div className="text-xs font-medium truncate">{f.name}</div>
                    <div className="text-[10px] text-slate-500">
                      {f.size_bytes} B • {f.modified}
                    </div>
                  </div>
                </button>
              );
            })}
          </>
        )}

        {activeTab === "plantillas" && (
          <div className="space-y-2.5">
            <div className="text-[11px] text-slate-400 px-1">
              Haz clic en una plantilla comentada en español para cargarla en el editor:
            </div>
            {FLOW_TEMPLATES_ES.map((tpl) => (
              <div
                key={tpl.id}
                onClick={() => onSelectTemplate(tpl)}
                className="p-3 rounded-xl bg-slate-900/60 hover:bg-slate-900 border border-slate-800 hover:border-emerald-500/40 cursor-pointer transition space-y-1.5"
              >
                <div className="flex items-center justify-between">
                  <span className="text-xs font-semibold text-emerald-300 flex items-center gap-1">
                    <Sparkles className="w-3.5 h-3.5" /> {tpl.nameEs}
                  </span>
                </div>
                <p className="text-[11px] text-slate-400 leading-relaxed">{tpl.descriptionEs}</p>
                <span className="inline-block text-[10px] px-2 py-0.5 rounded bg-slate-950 text-sky-300 border border-slate-800">
                  {tpl.category}
                </span>
              </div>
            ))}
          </div>
        )}

        {activeTab === "comandos" && (
          <div className="space-y-2">
            <div className="text-[11px] text-slate-400 px-1">
              Catálogo oficial de comandos Maestro documentados en español (clic para insertar):
            </div>
            {MAESTRO_COMMANDS_ES.map((cmd) => (
              <div
                key={cmd.id}
                onClick={() => onInsertCommand(cmd)}
                className="p-2.5 rounded-xl bg-slate-900/50 hover:bg-slate-900 border border-slate-800/80 hover:border-sky-500/40 cursor-pointer transition"
              >
                <div className="flex items-center justify-between mb-1">
                  <span className="text-xs font-semibold text-slate-100">{cmd.titleEs}</span>
                  <code className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-slate-950 text-emerald-400 border border-emerald-500/20">
                    {cmd.name}
                  </code>
                </div>
                <p className="text-[11px] text-slate-400 mb-1.5">{cmd.descriptionEs}</p>
                <pre className="text-[10px] font-mono bg-slate-950 p-1.5 rounded text-sky-300 border border-slate-800/80">
                  {cmd.snippet}
                </pre>
              </div>
            ))}
          </div>
        )}

        {activeTab === "variables" && (
          <div className="space-y-3">
            <div className="text-[11px] text-slate-400 px-1">
              Variables de entorno inyectadas en tus flujos (<code>{"${VARIABLE}"}</code>):
            </div>
            <div className="space-y-1.5">
              {Object.entries(envVars).map(([k, v]) => (
                <div
                  key={k}
                  className="p-2 rounded-lg bg-slate-900 border border-slate-800 flex items-center justify-between text-xs"
                >
                  <code className="text-emerald-400 font-mono">{k}</code>
                  <span className="text-slate-300 truncate max-w-[120px]">{v}</span>
                </div>
              ))}
            </div>
            <div className="pt-2 border-t border-slate-800 space-y-2">
              <input
                type="text"
                value={newEnvKey}
                onChange={(e) => setNewEnvKey(e.target.value.toUpperCase())}
                placeholder="NOMBRE_VARIABLE"
                className="w-full px-2.5 py-1.5 rounded-lg bg-slate-900 border border-slate-800 text-xs font-mono text-slate-100"
              />
              <input
                type="text"
                value={newEnvVal}
                onChange={(e) => setNewEnvVal(e.target.value)}
                placeholder="Valor de la variable"
                className="w-full px-2.5 py-1.5 rounded-lg bg-slate-900 border border-slate-800 text-xs text-slate-100"
              />
              <button
                onClick={() => {
                  if (newEnvKey.trim()) {
                    onUpdateEnvVar(newEnvKey.trim(), newEnvVal);
                    setNewEnvKey("");
                    setNewEnvVal("");
                  }
                }}
                className="w-full py-1.5 rounded-lg bg-emerald-500/20 hover:bg-emerald-500/30 text-emerald-300 border border-emerald-500/40 text-xs font-medium transition"
              >
                + Guardar Variable de Entorno
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Pie de Privacidad y Estado */}
      <div className="p-3 border-t border-slate-800/80 bg-slate-900/40 flex items-center gap-2 text-[11px] text-slate-400">
        <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />
        <span>Sin Sentry • Sin PostHog • 100% Local</span>
      </div>
    </div>
  );
};
