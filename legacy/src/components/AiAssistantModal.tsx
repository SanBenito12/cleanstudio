import React, { useState } from "react";
import { Sparkles, X, Cpu, Check, Wand2 } from "lucide-react";
import { generateAiFlow } from "../services/bridge";

interface Props {
  isOpen: boolean;
  onClose: () => void;
  currentYaml: string;
  onApplyYaml: (yaml: string) => void;
}

export const AiAssistantModal: React.FC<Props> = ({
  isOpen,
  onClose,
  currentYaml,
  onApplyYaml,
}) => {
  const [provider, setProvider] = useState<"offline" | "ollama" | "gemini" | "openai">("offline");
  const [model, setModel] = useState("qwen2.5-coder:7b");
  const [apiKey, setApiKey] = useState("");
  const [prompt, setPrompt] = useState(
    "Inicia sesión con usuario qa@cleanstudio.dev, busca un producto en el catálogo, agrégalo al carrito y toma captura de pantalla."
  );
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<{
    yaml: string;
    explanation: string;
    provider_used: string;
  } | null>(null);

  if (!isOpen) return null;

  const handleGenerate = async () => {
    setLoading(true);
    try {
      const res = await generateAiFlow({
        provider,
        model,
        api_key: apiKey || undefined,
        prompt,
        app_id: "com.ejemplo.tienda",
        current_yaml: currentYaml,
      });
      setResult(res);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/75 backdrop-blur-sm flex items-center justify-center p-6">
      <div className="w-full max-w-2xl rounded-2xl bg-slate-900 border border-slate-700/80 shadow-2xl overflow-hidden flex flex-col">
        {/* Cabecera */}
        <div className="px-6 py-4 bg-gradient-to-r from-emerald-950/80 via-slate-900 to-slate-900 border-b border-slate-800 flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <div className="p-2 rounded-xl bg-emerald-500/20 border border-emerald-500/40">
              <Sparkles className="w-5 h-5 text-emerald-400" />
            </div>
            <div>
              <h2 className="text-base font-bold text-white">
                Asistente IA Libre en Español — Clean Studio
              </h2>
              <p className="text-xs text-slate-400">
                Sin pagar Maestro Cloud • Compatible con Motor Offline Rust, Ollama Local, Gemini y OpenAI
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800 transition"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Cuerpo */}
        <div className="p-6 space-y-4 overflow-y-auto max-h-[75vh]">
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs font-medium text-slate-300 mb-1">
                Motor de Inteligencia Artificial
              </label>
              <select
                value={provider}
                onChange={(e) => setProvider(e.target.value as "offline" | "ollama" | "gemini" | "openai")}
                className="w-full px-3 py-2 rounded-lg bg-slate-950 border border-slate-800 text-xs text-slate-100 focus:outline-none focus:border-emerald-500"
              >
                <option value="offline">Motor Inteligente Local en Rust (Instantáneo / Sin API Key)</option>
                <option value="ollama">Ollama Local (http://localhost:11434 — 100% Privado)</option>
                <option value="gemini">Google Gemini API (BYOK)</option>
                <option value="openai">OpenAI GPT-4o API (BYOK)</option>
              </select>
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-300 mb-1">
                Modelo / Configuración
              </label>
              <input
                type="text"
                value={model}
                onChange={(e) => setModel(e.target.value)}
                placeholder="Ej. qwen2.5-coder:7b o gemini-2.5-flash"
                className="w-full px-3 py-2 rounded-lg bg-slate-950 border border-slate-800 text-xs text-slate-100 focus:outline-none focus:border-emerald-500"
              />
            </div>
          </div>

          {(provider === "gemini" || provider === "openai") && (
            <div>
              <label className="block text-xs font-medium text-slate-300 mb-1">
                Tu API Key ({provider.toUpperCase()}) — Guardada solo en memoria local
              </label>
              <input
                type="password"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                placeholder="Ingresa tu API Key..."
                className="w-full px-3 py-2 rounded-lg bg-slate-950 border border-slate-800 text-xs text-slate-100 focus:outline-none focus:border-emerald-500"
              />
            </div>
          )}

          <div>
            <label className="block text-xs font-medium text-slate-300 mb-1">
              Describe en español qué quieres automatizar o reparar:
            </label>
            <textarea
              rows={3}
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              className="w-full p-3 rounded-xl bg-slate-950 border border-slate-800 text-xs text-slate-100 focus:outline-none focus:border-emerald-500"
              placeholder="Ej: Inicia sesión con correo de prueba, navega al perfil y verifica que aparezca el nombre del usuario..."
            />
          </div>

          <button
            onClick={handleGenerate}
            disabled={loading}
            className="w-full py-2.5 rounded-xl bg-emerald-500 hover:bg-emerald-400 text-slate-950 font-semibold text-xs flex items-center justify-center gap-2 transition shadow-lg shadow-emerald-500/20"
          >
            <Wand2 className="w-4 h-4" />
            {loading ? "Generando Flujo YAML en Español..." : "Generar Flujo YAML con IA"}
          </button>

          {result && (
            <div className="mt-4 p-4 rounded-xl bg-slate-950 border border-emerald-500/30 space-y-3">
              <div className="flex items-center justify-between">
                <span className="text-xs font-semibold text-emerald-400 flex items-center gap-1.5">
                  <Cpu className="w-4 h-4" /> {result.provider_used}
                </span>
                <span className="text-[11px] text-slate-400">{result.explanation}</span>
              </div>
              <pre className="p-3 rounded-lg bg-slate-900 border border-slate-800 text-xs font-mono text-sky-300 overflow-x-auto max-h-52">
                {result.yaml}
              </pre>
              <div className="flex justify-end gap-2">
                <button
                  onClick={() => {
                    onApplyYaml(result.yaml);
                    onClose();
                  }}
                  className="px-4 py-2 rounded-lg bg-emerald-500 hover:bg-emerald-400 text-slate-950 font-semibold text-xs flex items-center gap-1.5 transition"
                >
                  <Check className="w-4 h-4" /> Reemplazar en el Editor YAML
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
