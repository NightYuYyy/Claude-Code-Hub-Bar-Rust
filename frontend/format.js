// Display formatters mirroring cch-core::format (used for rich tab rendering).

const fmt = {
  moneyParts(value) {
    if (!isFinite(value)) return ["$0.00", null];
    const abs = Math.abs(value);
    if (abs >= 1000) return [`$${(value / 1000).toFixed(1)}k`, null];
    if (abs >= 100) return [`$${value.toFixed(0)}`, null];
    let text = `$${value.toFixed(6)}`;
    text = text.replace(/0+$/, "").replace(/\.$/, "");
    if (text === "$0") text = "$0.00";
    const dot = text.indexOf(".");
    if (dot === -1) return [text, null];
    const frac = text.slice(dot + 1);
    if (frac.length <= 3) return [text, null];
    const major = `${text.slice(0, dot)}.${frac.slice(0, 3)}`;
    const minor = frac.slice(3);
    return [major, minor.length ? minor : null];
  },
  money(value) {
    const [a, b] = fmt.moneyParts(value);
    return b ? a + b : a;
  },
  moneyHtml(value) {
    const [a, b] = fmt.moneyParts(value);
    return b ? `${a}<span class="minor">${b}</span>` : a;
  },
  compact(value) {
    value = value || 0;
    if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
    if (value >= 1000) return `${(value / 1000).toFixed(1)}k`;
    return `${value}`;
  },
  percent(value) {
    return `${((value || 0) * 100).toFixed(1)}%`;
  },
  latency(ms) {
    if (!ms || ms <= 0) return "-";
    return `${(ms / 1000).toFixed(1)}s`;
  },
  msAsSeconds(ms) {
    if (ms === null || ms === undefined) return "-";
    return `${(ms / 1000).toFixed(2)}s`;
  },
  tokensPerSecond(v) {
    if (v && v > 0) return `${Math.round(v)} tok/s`;
    return "-- tok/s";
  },
  multiplier(value) {
    if (Math.abs(value - 1) < 0.001) return "x1";
    if (Math.round(value) === value) return `x${value.toFixed(0)}`;
    return `x${value.toFixed(2)}`;
  },
  duration(seconds) {
    const total = Math.max(0, Math.floor(seconds));
    if (total < 60) return `${total}s`;
    const m = Math.floor(total / 60);
    const s = total % 60;
    if (m < 60) return `${m}m${String(s).padStart(2, "0")}s`;
    const h = Math.floor(m / 60);
    return `${h}h${String(m % 60).padStart(2, "0")}m`;
  },
  elapsedSince(ms) {
    if (!ms) return "--";
    return fmt.duration(Math.max(0, (Date.now() - ms) / 1000));
  },
  relativeTime(iso) {
    if (!iso) return "";
    const t = Date.parse(iso.replace(" ", "T"));
    if (isNaN(t)) return iso;
    const diff = (Date.now() - t) / 1000;
    if (diff < 60) return `${Math.floor(diff)}s 前`;
    if (diff < 3600) return `${Math.floor(diff / 60)}m 前`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h 前`;
    return new Date(t).toLocaleDateString();
  },
  escape(text) {
    return String(text ?? "").replace(
      /[&<>"']/g,
      (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c])
    );
  },
};
