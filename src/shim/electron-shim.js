// Clean Studio — shim de Electron para ejecutar el renderer original de Maestro Studio en Tauri.
//
// El renderer original corre con nodeIntegration: usa require("electron").ipcRenderer y carga
// algunos módulos de Node al arrancar. Este script (clásico y síncrono, antes del bundle) emula
// esa superficie: los canales IPC van a los comandos Rust `ipc_invoke` / `ipc_send` y los
// mensajes main→renderer llegan como un único evento Tauri `cs-ipc` { channel, args }.
(function () {
  "use strict";

  var tauri = window.__TAURI__;
  if (!tauri) {
    console.error("[Clean Studio] window.__TAURI__ no disponible: ¿withGlobalTauri desactivado?");
    return;
  }

  // ---------------------------------------------------------------------------
  // ipcRenderer
  // ---------------------------------------------------------------------------
  var listeners = new Map(); // channel -> Set<fn(event, ...args)>
  var ready = false;
  var queue = [];

  function dispatch(channel, args) {
    var set = listeners.get(channel);
    if (!set) return;
    var event = { sender: ipcRenderer, senderId: 0, ports: [] };
    Array.from(set).forEach(function (fn) {
      try {
        fn.apply(null, [event].concat(args || []));
      } catch (err) {
        console.error("[Clean Studio] error en listener de " + channel, err);
      }
    });
  }

  function whenReady(fn) {
    if (ready) fn();
    else queue.push(fn);
  }

  tauri.event
    .listen("cs-ipc", function (e) {
      dispatch(e.payload.channel, e.payload.args);
    })
    .then(function () {
      ready = true;
      queue.splice(0).forEach(function (fn) {
        fn();
      });
    });

  function toError(err) {
    return err instanceof Error ? err : new Error(typeof err === "string" ? err : JSON.stringify(err));
  }

  var ipcRenderer = {
    send: function (channel) {
      var args = Array.prototype.slice.call(arguments, 1);
      whenReady(function () {
        tauri.core.invoke("ipc_send", { channel: channel, args: args }).catch(function (err) {
          console.error("[Clean Studio] ipc_send " + channel, err);
        });
      });
    },
    invoke: function (channel) {
      var args = Array.prototype.slice.call(arguments, 1);
      return new Promise(function (resolve, reject) {
        whenReady(function () {
          tauri.core
            .invoke("ipc_invoke", { channel: channel, args: args })
            .then(resolve, function (err) {
              reject(toError(err));
            });
        });
      });
    },
    sendSync: function (channel) {
      console.warn("[Clean Studio] sendSync no soportado: " + channel);
      return null;
    },
    on: function (channel, fn) {
      if (!listeners.has(channel)) listeners.set(channel, new Set());
      listeners.get(channel).add(fn);
      return ipcRenderer;
    },
    once: function (channel, fn) {
      var wrapper = function () {
        ipcRenderer.removeListener(channel, wrapper);
        fn.apply(null, arguments);
      };
      return ipcRenderer.on(channel, wrapper);
    },
    addListener: function (channel, fn) {
      return ipcRenderer.on(channel, fn);
    },
    removeListener: function (channel, fn) {
      var set = listeners.get(channel);
      if (set) set.delete(fn);
      return ipcRenderer;
    },
    off: function (channel, fn) {
      return ipcRenderer.removeListener(channel, fn);
    },
    removeAllListeners: function (channel) {
      if (channel) listeners.delete(channel);
      else listeners.clear();
      return ipcRenderer;
    },
    listenerCount: function (channel) {
      var set = listeners.get(channel);
      return set ? set.size : 0;
    },
    postMessage: function () {},
    sendTo: function () {},
    sendToHost: function () {},
  };

  function openExternal(url) {
    return ipcRenderer.invoke("open-external", String(url));
  }

  var electron = {
    ipcRenderer: ipcRenderer,
    shell: {
      openExternal: openExternal,
      openPath: function (p) {
        return ipcRenderer.invoke("show-in-file-manager", p).then(function () {
          return "";
        });
      },
      showItemInFolder: function (p) {
        ipcRenderer.invoke("show-in-file-manager", p);
      },
    },
    clipboard: {
      writeText: function (t) {
        navigator.clipboard && navigator.clipboard.writeText(String(t));
      },
      readText: function () {
        return "";
      },
    },
    contextBridge: { exposeInMainWorld: function () {} },
    webFrame: {
      setZoomFactor: function () {},
      getZoomFactor: function () {
        return 1;
      },
    },
    crashReporter: { start: function () {}, addExtraParameter: function () {} },
    nativeImage: { createFromDataURL: function () { return {}; } },
  };

  // ---------------------------------------------------------------------------
  // Módulos de Node que el bundle requiere al cargar (vite-plugin-electron-renderer
  // enumera sus exports). Basta con objetos inertes: su uso real (electron-store)
  // se reemplaza por CleanStudioStore.
  // ---------------------------------------------------------------------------
  function inert(name) {
    var fn = function () {
      return undefined;
    };
    var proxy = new Proxy(fn, {
      get: function (_t, prop) {
        if (prop === "default") return proxy;
        if (prop === "__esModule") return false;
        if (prop === Symbol.toPrimitive) return function () { return "[" + name + "]"; };
        if (prop === "then") return undefined;
        return inert(name + "." + String(prop));
      },
      apply: function () {
        return undefined;
      },
      construct: function () {
        return {};
      },
    });
    return proxy;
  }

  var pathPosix = {
    sep: "/",
    delimiter: ":",
    basename: function (p, ext) {
      var b = String(p).replace(/\/+$/, "").split("/").pop() || "";
      return ext && b.endsWith(ext) ? b.slice(0, -ext.length) : b;
    },
    dirname: function (p) {
      var s = String(p).replace(/\/+$/, "");
      var i = s.lastIndexOf("/");
      return i < 0 ? "." : i === 0 ? "/" : s.slice(0, i);
    },
    extname: function (p) {
      var b = pathPosix.basename(p);
      var i = b.lastIndexOf(".");
      return i <= 0 ? "" : b.slice(i);
    },
    isAbsolute: function (p) {
      return String(p).charAt(0) === "/";
    },
    normalize: function (p) {
      var abs = pathPosix.isAbsolute(p);
      var out = [];
      String(p)
        .split("/")
        .forEach(function (seg) {
          if (!seg || seg === ".") return;
          if (seg === "..") {
            if (out.length && out[out.length - 1] !== "..") out.pop();
            else if (!abs) out.push("..");
          } else out.push(seg);
        });
      return (abs ? "/" : "") + out.join("/") || (abs ? "/" : ".");
    },
    join: function () {
      return pathPosix.normalize(Array.prototype.filter.call(arguments, Boolean).join("/"));
    },
    resolve: function () {
      var parts = Array.prototype.slice.call(arguments);
      var acc = "";
      for (var i = parts.length - 1; i >= 0 && !pathPosix.isAbsolute(acc); i--) {
        acc = parts[i] + (acc ? "/" + acc : "");
      }
      return pathPosix.normalize(pathPosix.isAbsolute(acc) ? acc : "/" + acc);
    },
    relative: function (from, to) {
      var a = pathPosix.resolve(from).split("/").filter(Boolean);
      var b = pathPosix.resolve(to).split("/").filter(Boolean);
      var i = 0;
      while (i < a.length && i < b.length && a[i] === b[i]) i++;
      return a.slice(i).map(function () { return ".."; }).concat(b.slice(i)).join("/");
    },
  };
  pathPosix.posix = pathPosix;
  pathPosix.default = pathPosix;

  var processShim = {
    platform: "darwin",
    arch: "arm64",
    env: {},
    // Sin versions.node: así Monaco y otras librerías usan su rama de navegador
    versions: {},
    argv: [],
    type: "renderer",
    cwd: function () {
      return "/";
    },
    nextTick: function (fn) {
      var args = Array.prototype.slice.call(arguments, 1);
      Promise.resolve().then(function () {
        fn.apply(null, args);
      });
    },
    on: function () {},
    off: function () {},
    emit: function () {},
  };
  processShim.default = processShim;
  if (typeof window.process === "undefined") window.process = processShim;

  var utilShim = {
    TextDecoder: window.TextDecoder,
    TextEncoder: window.TextEncoder,
    inspect: function (v) {
      try {
        return JSON.stringify(v);
      } catch (_e) {
        return String(v);
      }
    },
    format: function () {
      return Array.prototype.map.call(arguments, String).join(" ");
    },
    isDeepStrictEqual: function (a, b) {
      return JSON.stringify(a) === JSON.stringify(b);
    },
    promisify: function (fn) {
      return fn;
    },
    inherits: function (ctor, superCtor) {
      Object.setPrototypeOf(ctor.prototype, superCtor.prototype);
    },
    deprecate: function (fn) {
      return fn;
    },
    types: {},
  };
  utilShim.default = utilShim;

  // conf/atomically leen userInfo() al cargar el módulo
  var osShim = {
    EOL: "\n",
    platform: function () {
      return "darwin";
    },
    type: function () {
      return "Darwin";
    },
    arch: function () {
      return "arm64";
    },
    release: function () {
      return "";
    },
    hostname: function () {
      return "localhost";
    },
    homedir: function () {
      return "/";
    },
    tmpdir: function () {
      return "/tmp";
    },
    cpus: function () {
      return [];
    },
    userInfo: function () {
      return { uid: -1, gid: -1, username: "", homedir: "/", shell: null };
    },
  };
  osShim.default = osShim;

  var modules = {
    electron: electron,
    path: pathPosix,
    "node:path": pathPosix,
    process: processShim,
    "node:process": processShim,
    util: utilShim,
    "node:util": utilShim,
    os: osShim,
    "node:os": osShim,
  };

  function cleanRequire(name) {
    if (Object.prototype.hasOwnProperty.call(modules, name)) return modules[name];
    if (/^(node:)?(fs|crypto|assert|os|events|stream|buffer|child_process|url)$/.test(name)) {
      return inert(name);
    }
    var err = new Error("Cannot find module '" + name + "'");
    err.code = "MODULE_NOT_FOUND";
    throw err;
  }
  cleanRequire.resolve = function (name) {
    return name;
  };
  cleanRequire.cache = {};
  window.require = cleanRequire;

  // ---------------------------------------------------------------------------
  // CleanStudioStore: sustituto de electron-store (get/set/has/delete/clear/store)
  // respaldado en localStorage. El bundle se parchea para usar `new CleanStudioStore`.
  // ---------------------------------------------------------------------------
  var STORE_KEY = "clean-studio:config";
  function readStore() {
    try {
      return JSON.parse(localStorage.getItem(STORE_KEY) || "{}") || {};
    } catch (_e) {
      return {};
    }
  }
  function writeStore(data) {
    try {
      localStorage.setItem(STORE_KEY, JSON.stringify(data));
    } catch (_e) {
      /* almacenamiento no disponible */
    }
  }
  function CleanStudioStore(opts) {
    this.path = "localStorage://" + STORE_KEY;
    var defaults = (opts && opts.defaults) || {};
    var data = readStore();
    Object.keys(defaults).forEach(function (k) {
      if (!(k in data)) data[k] = defaults[k];
    });
    writeStore(data);
  }
  CleanStudioStore.prototype.get = function (key, fallback) {
    var data = readStore();
    return key in data ? data[key] : fallback;
  };
  CleanStudioStore.prototype.set = function (key, value) {
    var data = readStore();
    if (key && typeof key === "object") Object.assign(data, key);
    else data[key] = value;
    writeStore(data);
  };
  CleanStudioStore.prototype.has = function (key) {
    return key in readStore();
  };
  CleanStudioStore.prototype.delete = function (key) {
    var data = readStore();
    delete data[key];
    writeStore(data);
  };
  CleanStudioStore.prototype.clear = function () {
    writeStore({});
  };
  CleanStudioStore.prototype.onDidChange = function () {
    return function () {};
  };
  CleanStudioStore.prototype.onDidAnyChange = function () {
    return function () {};
  };
  Object.defineProperty(CleanStudioStore.prototype, "store", {
    get: function () {
      return readStore();
    },
    set: function (v) {
      writeStore(v || {});
    },
  });
  Object.defineProperty(CleanStudioStore.prototype, "size", {
    get: function () {
      return Object.keys(readStore()).length;
    },
  });
  globalThis.CleanStudioStore = CleanStudioStore;

  // ---------------------------------------------------------------------------
  // Registro del renderer en el proceso nativo (como `console-message` del original)
  // ---------------------------------------------------------------------------
  function rendererLog(level, parts) {
    var text = Array.prototype.map
      .call(parts, function (p) {
        if (p instanceof Error) return p.name + ": " + p.message + "\n" + (p.stack || "");
        if (typeof p === "object") {
          try {
            return JSON.stringify(p);
          } catch (_e) {
            return String(p);
          }
        }
        return String(p);
      })
      .join(" ");
    tauri.core.invoke("ipc_send", { channel: "renderer-log", args: [level, text] }).catch(function () {});
  }
  ["error", "warn"].forEach(function (level) {
    var original = console[level];
    console[level] = function () {
      rendererLog(level, arguments);
      return original.apply(console, arguments);
    };
  });
  window.addEventListener("error", function (e) {
    rendererLog("error", [e.error || e.message]);
  });
  window.addEventListener("unhandledrejection", function (e) {
    rendererLog("error", ["Promesa rechazada:", e.reason]);
  });

  // ---------------------------------------------------------------------------
  // Enlaces externos: window.open(http…) abre el navegador del sistema
  // ---------------------------------------------------------------------------
  var nativeOpen = window.open;
  window.open = function (url, target, features) {
    if (typeof url === "string" && /^(https?:|mailto:)/i.test(url)) {
      openExternal(url);
      return null;
    }
    return nativeOpen.call(window, url, target, features);
  };
  document.addEventListener(
    "click",
    function (e) {
      var a = e.target && e.target.closest ? e.target.closest("a[href]") : null;
      if (!a) return;
      var href = a.getAttribute("href") || "";
      if (/^(https?:|mailto:)/i.test(href)) {
        e.preventDefault();
        openExternal(href);
      }
    },
    true,
  );

  // ---------------------------------------------------------------------------
  // Arrastre de ventana: el original usa -webkit-app-region (solo Electron)
  // ---------------------------------------------------------------------------
  var INTERACTIVE = "button,a,input,textarea,select,[role=button],[role=tab],[role=menuitem],[contenteditable=true]";
  function dragRegion(el) {
    for (var node = el; node && node !== document.documentElement; node = node.parentElement) {
      var cls = node.classList;
      if (!cls) continue;
      if (cls.contains("no-drag") || cls.contains("[-webkit-app-region:no-drag]")) return false;
      if (node.matches && node.matches(INTERACTIVE)) return false;
      if (cls.contains("drag") || cls.contains("[-webkit-app-region:drag]")) return true;
    }
    return false;
  }
  document.addEventListener("mousedown", function (e) {
    if (e.button !== 0 || !dragRegion(e.target)) return;
    if (e.detail === 2) {
      ipcRenderer.send("window-maximize");
      return;
    }
    tauri.window.getCurrentWindow().startDragging();
  });
})();
