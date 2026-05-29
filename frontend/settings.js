// Settings window controller.
const invoke = window.__TAURI__.core.invoke;
const emit = window.__TAURI__.event.emit;

let settings = null;

function applyToForm(s) {
  document.getElementById("set-url").value = s.cchBaseUrl || "";
  document.getElementById("set-token").value = s.cchToken || "";
  document.getElementById("set-env").value = s.cchEnvPath || "";
  const interval = s.refreshInterval || 15;
  document.getElementById("set-interval").value = interval;
  document.getElementById("interval-value").textContent = interval;
  setToggle("set-details", s.showStatusBarDetails);
  setToggle("set-updates", s.checkForUpdatesEnabled);
  setTheme(s.selectedTheme || "liquidGlass");
}

function setToggle(id, on) {
  document.getElementById(id).classList.toggle("on", !!on);
}
function toggleValue(id) {
  return document.getElementById(id).classList.contains("on");
}
function setTheme(value) {
  document.documentElement.dataset.theme = value;
  document.querySelectorAll(".theme-opt").forEach((el) => {
    el.classList.toggle("active", el.dataset.themeValue === value);
  });
}
function currentTheme() {
  return document.documentElement.dataset.theme || "liquidGlass";
}

function collect() {
  return {
    ...settings,
    cchBaseUrl: document.getElementById("set-url").value.trim(),
    cchToken: document.getElementById("set-token").value.trim(),
    cchEnvPath: document.getElementById("set-env").value.trim(),
    refreshInterval: parseFloat(document.getElementById("set-interval").value),
    showStatusBarDetails: toggleValue("set-details"),
    checkForUpdatesEnabled: toggleValue("set-updates"),
    selectedTheme: currentTheme(),
  };
}

document.addEventListener("click", async (event) => {
  const t = event.target;
  if (t.closest("#set-details")) return setToggle("set-details", !toggleValue("set-details"));
  if (t.closest("#set-updates")) return setToggle("set-updates", !toggleValue("set-updates"));

  const themeOpt = t.closest(".theme-opt");
  if (themeOpt) return setTheme(themeOpt.dataset.themeValue);

  if (t.closest("#btn-probe")) {
    const box = document.getElementById("probe-result");
    box.className = "probe-result";
    box.textContent = "测试中…";
    // Save first so the probe uses the entered connection.
    settings = collect();
    await invoke("save_settings", { settings });
    try {
      const r = await invoke("probe_connection");
      box.classList.add(r.ok ? "ok" : "err");
      box.textContent = r.ok
        ? `连接成功 · ${Math.round(r.latencyMs || 0)}ms`
        : `连接失败 · ${r.errorMessage || "未知错误"}`;
    } catch (e) {
      box.classList.add("err");
      box.textContent = `连接失败 · ${e}`;
    }
    return;
  }

  if (t.closest("#btn-save")) {
    settings = collect();
    await invoke("save_settings", { settings });
    await emit("settings-changed", {});
    window.__TAURI__.window.getCurrentWindow().close();
    return;
  }
  if (t.closest("#btn-cancel")) {
    window.__TAURI__.window.getCurrentWindow().close();
  }
});

document.getElementById("set-interval").addEventListener("input", (e) => {
  document.getElementById("interval-value").textContent = e.target.value;
});

(async function boot() {
  try {
    settings = await invoke("get_settings");
    applyToForm(settings);
  } catch (e) {
    console.error(e);
  }
})();
