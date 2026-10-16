import React, { useState } from "react";
import {
  Smartphone,
  Crosshair,
  Play,
  RotateCcw,
  Camera,
  CheckCircle2,
  MousePointerClick,
  Eye,
  Keyboard,
  Layers,
  Power,
  Wifi,
} from "lucide-react";
import { UIElementNode } from "../services/bridge";

interface Props {
  streamUrl: string | null;
  deviceName: string;
  platform: string;
  elements: UIElementNode[];
  selectedElement: UIElementNode | null;
  onSelectElement: (el: UIElementNode) => void;
  onInsertStep: (yamlSnippet: string) => void;
  onStartStream: () => void;
  onBootSimulator: () => void;
}

export const DeviceMirrorInspector: React.FC<Props> = ({
  streamUrl,
  deviceName,
  platform,
  elements,
  selectedElement,
  onSelectElement,
  onInsertStep,
  onStartStream,
  onBootSimulator,
}) => {
  const [inspectOverlay, setInspectOverlay] = useState(true);
  const [searchFilter, setSearchFilter] = useState("");

  const filteredElements = elements.filter((el) => {
    const q = searchFilter.toLowerCase();
    return (
      !q ||
      (el.resourceId && el.resourceId.toLowerCase().includes(q)) ||
      (el.text && el.text.toLowerCase().includes(q)) ||
      el.className.toLowerCase().includes(q)
    );
  });

  const buildTapSnippet = (el: UIElementNode) => {
    if (el.resourceId) {
      return `- tapOn:\n    id: "${el.resourceId}"`;
    }
    return `- tapOn: "${el.text || el.className}"`;
  };

  const buildAssertSnippet = (el: UIElementNode) => {
    if (el.text) {
      return `- assertVisible: "${el.text}"`;
    }
    return `- assertVisible:\n    id: "${el.resourceId || el.id}"`;
  };

  const buildInputSnippet = (el: UIElementNode) => {
    const target = el.resourceId
      ? `- tapOn:\n    id: "${el.resourceId}"`
      : `- tapOn: "${el.text || el.className}"`;
    return `${target}\n- inputText: "texto_ejemplo"`;
  };

  return (
    <div className="flex flex-col h-full bg-slate-950 border-l border-slate-800/80">
      {/* Encabezado del Inspector */}
      <div className="px-4 py-2.5 border-b border-slate-800/80 flex items-center justify-between bg-slate-900/50">
        <div className="flex items-center gap-2">
          <Smartphone className="w-4 h-4 text-emerald-400" />
          <span className="text-xs font-semibold tracking-wide text-slate-200 uppercase">
            Espejo en Vivo e Inspector UI
          </span>
        </div>
        <div className="flex items-center gap-1.5">
          <button
            onClick={() => setInspectOverlay(!inspectOverlay)}
            className={`px-2.5 py-1 rounded-md text-xs font-medium flex items-center gap-1 transition ${
              inspectOverlay
                ? "bg-emerald-500/20 text-emerald-300 border border-emerald-500/40"
                : "bg-slate-800 text-slate-400 border border-slate-700"
            }`}
            title="Activar/Desactivar recuadros de inspección"
          >
            <Crosshair className="w-3.5 h-3.5" />
            {inspectOverlay ? "Inspector ON" : "Inspector OFF"}
          </button>
          <button
            onClick={onStartStream}
            className="px-2.5 py-1 rounded-md text-xs font-medium bg-sky-500/20 text-sky-300 border border-sky-500/40 hover:bg-sky-500/30 transition flex items-center gap-1"
            title="Conectar stream nativo en Rust (60 FPS)"
          >
            <Wifi className="w-3.5 h-3.5" />
            Stream 60fps
          </button>
          <button
            onClick={onBootSimulator}
            className="p-1.5 rounded-md bg-slate-800 hover:bg-slate-700 text-slate-300 border border-slate-700 transition"
            title="Abrir / Encender Simulador Nativo"
          >
            <Power className="w-3.5 h-3.5 text-amber-400" />
          </button>
        </div>
      </div>

      {/* Marco del Dispositivo Móvil */}
      <div className="p-4 flex flex-col items-center justify-center bg-gradient-to-b from-slate-900/40 to-slate-950 border-b border-slate-800/80">
        <div className="relative w-[300px] h-[545px] rounded-[34px] border-[6px] border-slate-800 bg-slate-900 shadow-2xl overflow-hidden flex flex-col">
          {/* Isla Dinámica / Barra de Estado */}
          <div className="h-7 bg-slate-950 px-5 flex items-center justify-between text-[10px] text-slate-400 border-b border-slate-800/60">
            <span className="font-semibold text-slate-300">9:41</span>
            <div className="w-16 h-3.5 bg-black rounded-full border border-slate-800" />
            <span className="text-emerald-400 font-medium">{platform} • 5G</span>
          </div>

          {/* Lienzo de Pantalla o Stream MJPEG en Vivo */}
          <div className="relative flex-1 bg-slate-950 overflow-hidden">
            {streamUrl ? (
              <img
                src={streamUrl}
                alt="Stream en vivo del dispositivo"
                className="w-full h-full object-cover"
              />
            ) : (
              <div className="w-full h-full bg-gradient-to-br from-slate-900 via-slate-950 to-slate-900 p-4 flex flex-col justify-between">
                <div className="text-[11px] text-slate-400 flex items-center justify-between border-b border-slate-800/80 pb-2">
                  <span className="font-medium text-slate-200">{deviceName}</span>
                  <span className="text-emerald-400 text-[10px] bg-emerald-500/10 px-2 py-0.5 rounded-full border border-emerald-500/30">
                    Vista Interactiva Activa
                  </span>
                </div>
                <div className="my-auto text-center pointer-events-none opacity-20">
                  <Smartphone className="w-16 h-16 mx-auto mb-2 text-emerald-400" />
                  <p className="text-xs">Haz clic en cualquier elemento UI</p>
                </div>
                <div className="text-[10px] text-slate-500 text-center">
                  Haz clic sobre un componente para generar su paso YAML
                </div>
              </div>
            )}

            {/* Capa de Nodos Interactivos (ViewHierarchy) */}
            {inspectOverlay &&
              elements.map((el) => {
                const isSelected = selectedElement?.id === el.id;
                return (
                  <button
                    key={el.id}
                    onClick={() => onSelectElement(el)}
                    style={{
                      left: `${el.bounds.x}px`,
                      top: `${el.bounds.y - 25}px`,
                      width: `${el.bounds.width}px`,
                      height: `${el.bounds.height}px`,
                    }}
                    className={`absolute rounded-lg text-left px-2.5 py-1 transition-all flex flex-col justify-center group ${
                      isSelected
                        ? "border-2 border-emerald-400 bg-emerald-500/20 shadow-lg shadow-emerald-500/20 z-20"
                        : "border border-sky-400/50 bg-slate-900/75 hover:border-emerald-400 hover:bg-emerald-500/15 z-10"
                    }`}
                  >
                    <div className="flex items-center justify-between gap-1">
                      <span className="text-xs font-medium text-slate-100 truncate">
                        {el.text || el.resourceId}
                      </span>
                      {el.resourceId && (
                        <span className="text-[9px] font-mono px-1.5 py-0.5 rounded bg-slate-950/90 text-emerald-300 border border-emerald-500/30 shrink-0">
                          #{el.resourceId}
                        </span>
                      )}
                    </div>
                  </button>
                );
              })}
          </div>

          {/* Barra Inferior de Controles Físicos */}
          <div className="h-9 bg-slate-950 border-t border-slate-800 px-4 flex items-center justify-around text-xs text-slate-400">
            <button
              onClick={() => onInsertStep("- back")}
              className="hover:text-white transition flex items-center gap-1"
              title="Insertar botón Atrás"
            >
              <RotateCcw className="w-3.5 h-3.5" /> Atrás
            </button>
            <button
              onClick={() => onInsertStep("- launchApp:\n    clearState: true")}
              className="hover:text-white transition flex items-center gap-1"
              title="Insertar Reinicio de App"
            >
              <Play className="w-3.5 h-3.5 text-emerald-400" /> App
            </button>
            <button
              onClick={() => onInsertStep('- takeScreenshot: "captura-pantalla"')}
              className="hover:text-white transition flex items-center gap-1"
              title="Insertar captura de pantalla"
            >
              <Camera className="w-3.5 h-3.5 text-sky-400" /> Captura
            </button>
          </div>
        </div>

        {/* Acciones Rápidas del Elemento Seleccionado */}
        {selectedElement && (
          <div className="w-full mt-3 p-3 rounded-xl bg-slate-900/90 border border-emerald-500/30 shadow-lg">
            <div className="flex items-center justify-between mb-2">
              <div className="flex items-center gap-1.5">
                <CheckCircle2 className="w-4 h-4 text-emerald-400" />
                <span className="text-xs font-semibold text-slate-200">
                  Elemento seleccionado:{" "}
                  <code className="text-emerald-300 font-mono">
                    {selectedElement.resourceId || selectedElement.text}
                  </code>
                </span>
              </div>
              <span className="text-[10px] font-mono text-slate-400 bg-slate-950 px-2 py-0.5 rounded">
                {selectedElement.className}
              </span>
            </div>
            <div className="grid grid-cols-3 gap-1.5">
              <button
                onClick={() => onInsertStep(buildTapSnippet(selectedElement))}
                className="px-2.5 py-1.5 rounded-lg bg-emerald-500/20 hover:bg-emerald-500/30 text-emerald-300 border border-emerald-500/30 text-xs font-medium flex items-center justify-center gap-1 transition"
              >
                <MousePointerClick className="w-3.5 h-3.5" /> + Tocar (tapOn)
              </button>
              <button
                onClick={() => onInsertStep(buildAssertSnippet(selectedElement))}
                className="px-2.5 py-1.5 rounded-lg bg-sky-500/20 hover:bg-sky-500/30 text-sky-300 border border-sky-500/30 text-xs font-medium flex items-center justify-center gap-1 transition"
              >
                <Eye className="w-3.5 h-3.5" /> + Verificar
              </button>
              <button
                onClick={() => onInsertStep(buildInputSnippet(selectedElement))}
                className="px-2.5 py-1.5 rounded-lg bg-purple-500/20 hover:bg-purple-500/30 text-purple-300 border border-purple-500/30 text-xs font-medium flex items-center justify-center gap-1 transition"
              >
                <Keyboard className="w-3.5 h-3.5" /> + Escribir
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Lista del Árbol de Jerarquía UI */}
      <div className="flex-1 flex flex-col min-h-0">
        <div className="px-4 py-2 bg-slate-900/60 border-b border-slate-800/80 flex items-center justify-between gap-2">
          <div className="flex items-center gap-1.5 text-xs font-semibold text-slate-300">
            <Layers className="w-3.5 h-3.5 text-sky-400" />
            <span>Jerarquía de Elementos ({filteredElements.length})</span>
          </div>
          <input
            type="text"
            value={searchFilter}
            onChange={(e) => setSearchFilter(e.target.value)}
            placeholder="Filtrar por ID o texto..."
            className="px-2.5 py-1 rounded-md bg-slate-950 border border-slate-800 text-xs text-slate-200 placeholder-slate-500 focus:outline-none focus:border-emerald-500 w-44"
          />
        </div>
        <div className="flex-1 overflow-y-auto p-2 space-y-1">
          {filteredElements.map((el) => {
            const active = selectedElement?.id === el.id;
            return (
              <div
                key={el.id}
                onClick={() => onSelectElement(el)}
                className={`p-2 rounded-lg border cursor-pointer transition flex items-center justify-between gap-2 ${
                  active
                    ? "bg-emerald-500/15 border-emerald-500/40 text-slate-100"
                    : "bg-slate-900/40 border-slate-800/60 hover:bg-slate-900 text-slate-300"
                }`}
              >
                <div className="min-w-0">
                  <div className="text-xs font-medium truncate">
                    {el.text || <span className="italic text-slate-500">(Sin texto)</span>}
                  </div>
                  <div className="text-[10px] font-mono text-slate-400 truncate">
                    id: <span className="text-emerald-400">{el.resourceId || "N/A"}</span> • {el.className}
                  </div>
                </div>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onInsertStep(buildTapSnippet(el));
                  }}
                  className="px-2 py-1 rounded bg-slate-800 hover:bg-emerald-500/20 hover:text-emerald-300 text-[10px] font-medium text-slate-300 shrink-0 transition"
                >
                  + Insertar
                </button>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
};
