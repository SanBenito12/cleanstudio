// Ventana "Claude en Clean Studio": estado de la conexión y acciones (backend: src-tauri/src/claude.rs)
(function () {
  "use strict";
  var invoke = function (channel, args) {
    return window.__TAURI__.core.invoke("ipc_invoke", { channel: channel, args: args || [] });
  };
  var $ = function (id) {
    return document.getElementById(id);
  };

  var EJEMPLOS = [
    "Lista mis dispositivos y conecta el iPhone 17.",
    "Mira la pantalla del simulador y dime qué elementos puedo tocar.",
    "Escribe un flujo que abra Ajustes, entre a General y tome una captura. Guárdalo en mi carpeta de pruebas y ejecútalo.",
    "¿Por qué falló la última ejecución de mi workspace? Explícamelo y corrige el flujo.",
  ];

  function toast(text) {
    var t = $("toast");
    t.textContent = text;
    t.classList.add("show");
    clearTimeout(toast.timer);
    toast.timer = setTimeout(function () {
      t.classList.remove("show");
    }, 2200);
  }

  function copy(text) {
    invoke("cs-claude-copy", [text]).then(function () {
      toast("Copiado");
    });
  }

  var list = $("prompts");
  EJEMPLOS.forEach(function (texto) {
    var row = document.createElement("div");
    row.className = "prompt";
    var span = document.createElement("span");
    span.textContent = texto;
    var btn = document.createElement("button");
    btn.className = "ghost";
    btn.textContent = "Copiar";
    btn.addEventListener("click", function () {
      copy(texto);
    });
    row.appendChild(span);
    row.appendChild(btn);
    list.appendChild(row);
  });

  function pill(el, kind, text) {
    el.className = "pill " + kind;
    el.textContent = text;
  }

  function render(s) {
    var d = s.desktop;
    var c = s.code;
    var srv = s.servidor;

    // Claude Desktop
    var dBtn = $("desktop-btn");
    dBtn.disabled = false;
    if (!d.instalado) {
      pill($("desktop-pill"), "muted", "No instalado");
      $("desktop-text").textContent = "Descárgalo en claude.ai/download para usar Clean Studio desde el chat.";
      dBtn.textContent = "Conectar";
      dBtn.disabled = true;
    } else if (d.conectado) {
      pill($("desktop-pill"), "ok", "Conectado");
      $("desktop-text").textContent = "Listo. Si acabas de conectarlo, cierra y vuelve a abrir Claude Desktop.";
      dBtn.textContent = "Abrir Claude Desktop";
      dBtn.className = "ghost";
      dBtn.dataset.action = "open";
    } else {
      pill($("desktop-pill"), "warn", d.otraRuta ? "Apunta a otra copia" : "Sin conectar");
      $("desktop-text").textContent = d.otraRuta
        ? "Claude Desktop tiene registrada otra copia de Clean Studio. Reconéctalo a esta."
        : "Con un clic agrego Clean Studio a tu Claude Desktop.";
      dBtn.textContent = d.otraRuta ? "Reconectar" : "Conectar";
      dBtn.className = "";
      dBtn.dataset.action = "add";
    }

    // Claude Code
    var cBtn = $("code-btn");
    $("code-cmd").textContent = c.comando;
    // El comando solo se muestra cuando hay que ejecutarlo a mano; si no, basta con "Copiar comando"
    $("code-cmd").hidden = c.instalado;
    cBtn.disabled = false;
    if (c.conectado) {
      pill($("code-pill"), "ok", "Conectado");
      $("code-text").textContent = "Listo en cualquier carpeta. Escribe claude en la terminal y pídele lo que necesites.";
      cBtn.textContent = "Copiar comando";
      cBtn.className = "ghost";
      cBtn.dataset.action = "copy";
    } else if (c.instalado) {
      pill($("code-pill"), "warn", "Sin conectar");
      $("code-text").textContent = "Con un clic lo registro en Claude Code para todas tus carpetas.";
      cBtn.textContent = "Conectar";
      cBtn.className = "";
      cBtn.dataset.action = "add";
    } else {
      pill($("code-pill"), "muted", "No instalado");
      $("code-text").textContent = "Si instalas Claude Code, ejecuta este comando una vez:";
      cBtn.textContent = "Copiar comando";
      cBtn.className = "ghost";
      cBtn.dataset.action = "copy";
    }

    // Servidor MCP
    if (srv.ok) {
      pill($("server-pill"), "ok", "Funciona");
      $("server-text").textContent = "Respondió a una prueba real con " + srv.herramientas + " herramientas.";
    } else {
      pill($("server-pill"), "bad", "No responde");
      $("server-text").textContent = "La prueba falló (" + (srv.error || "sin respuesta") + "). Vuelve a instalar Clean Studio.";
    }

    // Resumen: dónde funciona ya y qué falta, sin dar por buena una conexión pendiente
    var banner = $("banner");
    var listos = [d.conectado && "Claude Desktop", c.conectado && "Claude Code"].filter(Boolean);
    var pendientes = [];
    if (d.instalado && !d.conectado) {
      pendientes.push(d.otraRuta ? "Claude Desktop necesita reconectarse" : "Falta conectar Claude Desktop");
    }
    if (c.instalado && !c.conectado) pendientes.push("Falta conectar Claude Code");
    var titulo, detalle;
    if (!srv.ok) {
      banner.className = "banner bad";
      titulo = "El servidor para Claude no responde";
      detalle = "Revisa el detalle abajo.";
    } else if (listos.length && !pendientes.length) {
      banner.className = "banner ok";
      titulo = "Todo listo";
      detalle = "Puedes usar Clean Studio desde " + listos.join(" y ") + ".";
    } else if (listos.length) {
      banner.className = "banner warn";
      titulo = "Listo en " + listos.join(" y ");
      detalle = pendientes.join(". ") + ": usa el botón de abajo.";
    } else {
      banner.className = "banner warn";
      titulo = "Falta conectar";
      detalle = "Conecta Claude Desktop o Claude Code con los botones de abajo.";
    }
    banner.innerHTML = '<span class="dot"></span><div><span class="titulo"></span><small></small></div>';
    banner.querySelector(".titulo").textContent = titulo;
    banner.querySelector("small").textContent = detalle;
  }

  function refresh() {
    $("refresh").disabled = true;
    invoke("cs-claude-status")
      .then(render)
      .catch(function (e) {
        toast(String(e));
      })
      .finally(function () {
        $("refresh").disabled = false;
      });
  }

  function act(btn, channel, okText) {
    btn.disabled = true;
    invoke(channel)
      .then(function () {
        toast(okText);
        refresh();
      })
      .catch(function (e) {
        btn.disabled = false;
        toast(String(e));
      });
  }

  $("desktop-btn").addEventListener("click", function () {
    var b = $("desktop-btn");
    if (b.dataset.action === "open") invoke("cs-claude-open-desktop");
    else act(b, "cs-claude-add-desktop", "Conectado. Reinicia Claude Desktop.");
  });
  $("code-btn").addEventListener("click", function () {
    var b = $("code-btn");
    if (b.dataset.action === "copy") copy($("code-cmd").textContent);
    else act(b, "cs-claude-add-code", "Conectado a Claude Code.");
  });
  $("refresh").addEventListener("click", refresh);
  refresh();
})();
