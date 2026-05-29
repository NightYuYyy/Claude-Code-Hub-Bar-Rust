// Tab renderers. Each takes the view-model and returns an HTML string.

const render = {
  dashboard(vm) {
    const o = vm.overview || {};
    const reqDelta = deltaPill(o.todayRequests, o.yesterdaySamePeriodRequests, false);
    const costDelta = deltaPill(o.todayCost, o.yesterdaySamePeriodCost, true);
    const errPct = fmt.percent(o.todayErrorRate || 0);
    const metrics = `
      <div class="metric-grid">
        ${metric("今日花费", fmt.moneyHtml(o.todayCost || 0), costDelta)}
        ${metric("今日请求", fmt.compact(o.todayRequests || 0), reqDelta)}
        ${metric("并发会话", `${o.concurrentSessions || 0}`)}
        ${metric("平均响应", fmt.latency(o.avgResponseTime || 0))}
        ${metric("近 1 分钟", `${o.recentMinuteRequests || 0} <span class="minor">req</span>`)}
        ${metric("错误率", errPct, o.todayErrorRate > 0.05 ? `<span class="pill negative">偏高</span>` : `<span class="pill positive">正常</span>`)}
      </div>`;

    const running = (vm.menuBarRunningLogs || []);
    const runningCard = running.length
      ? `<div class="section-title">进行中 (${running.length})</div>` +
        running
          .map((log) => {
            const cache = (vm.cacheStatus || {})[String(log.id)];
            const rebuilding = cache && cache.state === "rebuilding";
            const model = log.model || log.originalModel || "model";
            return `<div class="card running-card">
              <div class="running-head">
                <span class="live-dot"></span>
                <strong>${fmt.escape(log.providerName || "Provider")}</strong>
                <span class="pill muted">${fmt.escape(model)}</span>
                ${rebuilding ? '<span class="cache-dot rebuilding" title="缓存重建中"></span>' : ""}
                <span class="spacer" style="flex:1"></span>
                <span class="mono" data-elapsed="${log.createdAt ? Date.parse((log.createdAt||'').replace(' ','T')) : ''}">${fmt.relativeTime(log.createdAt)}</span>
              </div>
              <div class="row-sub">${fmt.escape(log.userName || "")} · ${fmt.escape(log.keyName || "")}</div>
            </div>`;
          })
          .join("")
      : "";

    const sessions = (vm.activeSessions || []).slice(0, 6);
    const sessionRows = sessions.length
      ? `<div class="section-title">活跃会话</div>` +
        sessions
          .map(
            (s) => `<div class="row">
              <div class="row-main">
                <div class="row-title">${fmt.escape(s.providerName || s.model || "Session")}</div>
                <div class="row-sub">${fmt.escape(s.userName || "")} · ${fmt.compact(s.totalTokens || 0)} tok</div>
              </div>
              <div class="row-trailing">
                <div class="primary">${fmt.money(s.costUsd || 0)}</div>
                <div class="secondary">${fmt.compact(s.requestCount || 0)} req</div>
              </div>
            </div>`
          )
          .join("")
      : "";

    return metrics + runningCard + sessionRows || metrics;
  },

  leaderboard(vm) {
    const summary = vm.leaderboardSummary || {};
    const entries = vm.leaderboard || [];
    const header = `
      <div class="controls-row">
        <div class="segmented" data-group="period">
          ${segBtn("daily", "今日", vm._period)}
          ${segBtn("weekly", "本周", vm._period)}
          ${segBtn("monthly", "本月", vm._period)}
        </div>
      </div>
      <div class="controls-row">
        <div class="segmented" data-group="scope">
          ${segBtn("user", "用户", vm._scope)}
          ${segBtn("provider", "供应商", vm._scope)}
          ${segBtn("model", "模型", vm._scope)}
        </div>
      </div>
      <div class="metric-grid">
        ${metric("总请求", fmt.compact(summary.requests || 0))}
        ${metric("总花费", fmt.moneyHtml(summary.cost || 0))}
        ${metric("总 Tokens", fmt.compact(summary.tokens || 0))}
        ${metric("缓存命中", summary.cacheHitRate != null ? fmt.percent(summary.cacheHitRate) : "—")}
      </div>`;

    if (!entries.length) return header + `<div class="empty">暂无排行数据</div>`;
    const maxCost = Math.max(...entries.map((e) => e.cost || 0), 0.000001);
    const rows = entries
      .map((e, i) => {
        const rankClass = i === 0 ? "gold" : i === 1 ? "silver" : i === 2 ? "bronze" : "";
        const hit = e.cacheHitRateOverride != null ? `<span class="pill muted">命中 ${fmt.percent(e.cacheHitRateOverride)}</span>` : "";
        return `<div class="row">
          <div class="rank ${rankClass}">${i + 1}</div>
          <div class="row-main">
            <div class="row-title">${fmt.escape(e.title)} ${hit}</div>
            <div class="row-sub">${fmt.compact(e.requests || 0)} req · ${fmt.compact(e.tokens || 0)} tok</div>
            <div class="bar"><span style="width:${Math.round(((e.cost || 0) / maxCost) * 100)}%"></span></div>
          </div>
          <div class="row-trailing">
            <div class="primary">${fmt.money(e.cost || 0)}</div>
          </div>
        </div>`;
      })
      .join("");
    return header + rows;
  },
};

render.logs = function (vm) {
  const logs = vm.logs && vm.logs.length ? vm.logs : vm.recentLogs || [];
  const s = vm.logSummary || {};
  const header = `
    <div class="controls-row">
      <input type="text" id="logs-model" placeholder="模型过滤" value="${fmt.escape(vm._logModel || "")}" />
      <input type="text" id="logs-status" placeholder="状态码" value="${fmt.escape(vm._logStatus || "")}" />
    </div>
    <div class="metric-grid">
      ${metric("请求", fmt.compact(s.totalRequests || vm.logTotal || logs.length))}
      ${metric("花费", fmt.moneyHtml(s.totalCost || 0))}
      ${metric("输入", fmt.compact(s.inputTokens || 0))}
      ${metric("缓存读", fmt.compact(s.cacheReadTokens || 0))}
    </div>`;
  if (!logs.length) return header + `<div class="empty">暂无日志</div>`;
  const rows = logs
    .map((log) => {
      const code = log.statusCode;
      const ok = code == null ? null : code >= 200 && code < 300;
      const statusPill =
        code == null
          ? `<span class="pill warning">运行中</span>`
          : `<span class="pill ${ok ? "positive" : "negative"}">${code}</span>`;
      const cache = (vm.cacheStatus || {})[String(log.id)];
      const rebuilding = cache && cache.state === "rebuilding";
      const model = log.model || log.originalModel || "model";
      const fast = log.isFastTier ? `<span class="pill">Fast</span>` : "";
      const tps = fmt.tokensPerSecond(log.tokensPerSecond);
      return `<div class="row">
        <div class="row-main">
          <div class="row-title">${fmt.escape(model)} ${fast} ${rebuilding ? '<span class="cache-dot rebuilding"></span>' : ""}</div>
          <div class="row-sub">${fmt.escape(log.providerName || "")} · ${fmt.escape(log.userName || "")} · ${fmt.relativeTime(log.createdAt)}</div>
          <div class="row-sub">${fmt.compact(log.inputTokens || 0)}↑ ${fmt.compact(log.outputTokens || 0)}↓ · ${tps} · ${fmt.msAsSeconds(log.durationMs)}</div>
        </div>
        <div class="row-trailing">
          ${statusPill}
          <div class="primary" style="margin-top:4px">${fmt.money(log.costUsd || 0)}</div>
        </div>
      </div>`;
    })
    .join("");
  return header + rows;
};

render.providers = function (vm) {
  const providers = vm.providers || [];
  const groups = vm.providerGroups || ["全部"];
  const active = vm._providerGroup || "全部";
  const filtered =
    active === "全部"
      ? providers
      : providers.filter((p) => providerTitles(p.groupTag).includes(active));
  const chips = groups
    .map(
      (g) =>
        `<button data-value="${fmt.escape(g)}" class="${g === active ? "active" : ""}">${fmt.escape(g)}</button>`
    )
    .join("");
  const header = `<div class="controls-row"><div class="segmented" data-group="providerGroup" style="flex-wrap:wrap">${chips}</div></div>`;
  if (!filtered.length) return header + `<div class="empty">暂无供应商</div>`;
  const rows = filtered
    .map((p) => {
      const circuit = (p.health && p.health.circuitState) || "closed";
      const open = circuit !== "closed";
      const statusPill = !p.isEnabled
        ? `<span class="pill muted">已停用</span>`
        : open
        ? `<span class="pill negative">熔断</span>`
        : `<span class="pill positive">正常</span>`;
      const mult = fmt.multiplier(p.costMultiplier || 1);
      return `<div class="row">
        <div class="row-main">
          <div class="row-title">${fmt.escape(p.name)} <span class="pill muted">${mult}</span></div>
          <div class="row-sub">${fmt.escape(providerTitles(p.groupTag).join(" / "))} · ${fmt.escape(p.limitText || "")}</div>
          <div class="row-sub">今日 ${fmt.compact(p.todayCalls || 0)} 次 · ${fmt.money(p.todayCost || 0)}</div>
        </div>
        <div class="row-trailing">
          ${statusPill}
          <div class="secondary" style="margin-top:6px">
            <span class="toggle ${p.isEnabled ? "on" : ""}" data-provider-toggle="${p.id}" data-enabled="${p.isEnabled}"></span>
          </div>
          ${open ? `<button class="pill" data-reset-circuit="${p.id}" style="margin-top:6px">重置</button>` : ""}
        </div>
      </div>`;
    })
    .join("");
  return header + rows;
};

function providerTitles(groupTag) {
  const trimmed = (groupTag || "").trim();
  if (!trimmed || isDefaultGroup(trimmed)) return ["默认"];
  const parts = trimmed
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s && !isDefaultGroup(s));
  return parts.length ? parts : ["默认"];
}
function isDefaultGroup(v) {
  const n = (v || "").trim().toLowerCase();
  return n === "" || n === "默认" || n === "default";
}

function metric(label, value, extra) {
  return `<div class="metric">
    <div class="label">${label}</div>
    <div class="value">${value}</div>
    ${extra ? `<div class="delta">${extra}</div>` : ""}
  </div>`;
}

function segBtn(value, label, active) {
  return `<button data-value="${value}" class="${active === value ? "active" : ""}">${label}</button>`;
}

function deltaPill(today, yesterday, isMoney) {
  today = today || 0;
  yesterday = yesterday || 0;
  if (yesterday <= 0) return "";
  const diff = today - yesterday;
  const pct = (diff / yesterday) * 100;
  const cls = diff >= 0 ? "positive" : "negative";
  const arrow = diff >= 0 ? "▲" : "▼";
  return `<span class="pill ${cls}">${arrow} ${Math.abs(pct).toFixed(0)}%</span>`;
}
