// Panel controller: subscribes to view-model events, renders tabs, and wires
// user interactions to Tauri commands.

const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;

const state = {
  vm: null,
  tab: "dashboard",
  period: "daily",
  scope: "user",
  providerGroup: "全部",
  logModel: "",
  logStatus: "",
  version: "1.1.12",
};

function decorate(vm) {
  vm._period = state.period;
  vm._scope = state.scope;
  vm._providerGroup = state.providerGroup;
  vm._logModel = state.logModel;
  vm._logStatus = state.logStatus;
  return vm;
}

function renderActiveTab() {
  if (!state.vm) return;
  const vm = decorate(state.vm);
  const panel = document.getElementById(`tab-${state.tab}`);
  if (panel && render[state.tab]) {
    panel.innerHTML = render[state.tab](vm);
  }
  // Connection subtitle + error banner + footer.
  const err = vm.errorMessage;
  const banner = document.getElementById("error-banner");
  if (err) {
    banner.textContent = err;
    banner.classList.remove("hidden");
    document.getElementById("conn-subtitle").textContent = "连接异常";
  } else {
    banner.classList.add("hidden");
    document.getElementById("conn-subtitle").textContent =
      (vm.statusBar && vm.statusBar.idlePrimary) || "已连接";
  }
  document.getElementById("footer-status").textContent = err ? "错误" : "就绪";
}

function applyViewModel(vm) {
  state.vm = vm;
  if (vm.statusBar) {
    // keep period/scope synced if backend changed
  }
  renderActiveTab();
}

function switchTab(tab) {
  state.tab = tab;
  document.querySelectorAll(".tab").forEach((el) => {
    el.classList.toggle("active", el.dataset.tab === tab);
  });
  document.querySelectorAll(".tab-panel").forEach((el) => {
    el.classList.toggle("active", el.id === `tab-${tab}`);
  });
  renderActiveTab();
}

async function refreshNow() {
  const btn = document.getElementById("btn-refresh");
  btn.classList.add("loading");
  try {
    const vm = await invoke("refresh_now");
    applyViewModel(vm);
  } catch (e) {
    console.error(e);
  } finally {
    setTimeout(() => btn.classList.remove("loading"), 400);
  }
}

async function changeLeaderboard(period, scope) {
  state.period = period;
  state.scope = scope;
  try {
    const vm = await invoke("set_leaderboard", { period, scope });
    applyViewModel(vm);
  } catch (e) {
    console.error(e);
  }
}

async function applyLogFilters() {
  try {
    const page = await invoke("fetch_logs", {
      page: 1,
      pageSize: 50,
      model: state.logModel,
      statusCode: state.logStatus,
      sessionId: "",
      includeStats: true,
    });
    if (state.vm) {
      state.vm.logs = page.logs;
      state.vm.logSummary = page.summary;
      state.vm.logTotal = page.total;
      renderActiveTab();
    }
  } catch (e) {
    console.error(e);
  }
}

// ---- Event delegation ----

document.addEventListener("click", async (event) => {
  const target = event.target;

  const tab = target.closest(".tab");
  if (tab) return switchTab(tab.dataset.tab);

  if (target.closest("#btn-refresh")) return refreshNow();
  if (target.closest("#btn-settings")) return invoke("open_settings_window");
  if (target.closest("#btn-update"))
    return window.__TAURI__.core.invoke("check_for_updates").catch(() => {});

  const seg = target.closest(".segmented button");
  if (seg) {
    const group = seg.parentElement.dataset.group;
    const value = seg.dataset.value;
    if (group === "period") return changeLeaderboard(value, state.scope);
    if (group === "scope") return changeLeaderboard(state.period, value);
    if (group === "providerGroup") {
      state.providerGroup = value;
      return renderActiveTab();
    }
  }

  const toggle = target.closest("[data-provider-toggle]");
  if (toggle) {
    const id = parseInt(toggle.dataset.providerToggle, 10);
    const enabled = toggle.dataset.enabled === "true";
    try {
      await invoke("set_provider_enabled", { providerId: id, enabled: !enabled });
    } catch (e) {
      console.error(e);
    }
    return;
  }

  const reset = target.closest("[data-reset-circuit]");
  if (reset) {
    const id = parseInt(reset.dataset.resetCircuit, 10);
    try {
      await invoke("reset_provider_circuit", { providerId: id });
    } catch (e) {
      console.error(e);
    }
    return;
  }
});

document.addEventListener("change", (event) => {
  if (event.target.id === "logs-model") {
    state.logModel = event.target.value;
    applyLogFilters();
  }
  if (event.target.id === "logs-status") {
    state.logStatus = event.target.value;
    applyLogFilters();
  }
});

// Live-update elapsed timers each second.
setInterval(() => {
  document.querySelectorAll("[data-elapsed]").forEach((el) => {
    const ms = parseInt(el.dataset.elapsed, 10);
    if (!isNaN(ms)) el.textContent = fmt.relativeTime(new Date(ms).toISOString());
  });
}, 1000);

async function boot() {
  // Theme + version from settings.
  try {
    const settings = await invoke("get_settings");
    document.documentElement.dataset.theme = settings.selectedTheme || "liquidGlass";
    state.period = settings.leaderboardPeriod || "daily";
    state.scope = settings.leaderboardScope || "user";
  } catch (e) {
    console.error(e);
  }
  try {
    const vm = await invoke("get_view_model");
    applyViewModel(vm);
  } catch (e) {
    console.error(e);
  }
  await listen("view-model", (event) => applyViewModel(event.payload));
  await listen("settings-changed", async () => {
    try {
      const settings = await invoke("get_settings");
      document.documentElement.dataset.theme = settings.selectedTheme || "liquidGlass";
    } catch (e) {
      console.error(e);
    }
  });
  // Check for updates (non-blocking).
  invoke("check_for_updates")
    .then((release) => {
      if (release) {
        const btn = document.getElementById("btn-update");
        btn.classList.remove("hidden");
        btn.title = release.name || release.tag;
        btn.onclick = () => window.__TAURI__.core.invoke("open_settings_window");
      }
    })
    .catch(() => {});
}

boot();
