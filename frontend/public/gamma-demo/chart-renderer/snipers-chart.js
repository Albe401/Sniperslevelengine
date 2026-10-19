function ee(l, c) {
  if (!Number.isInteger(c) || c <= 0) throw new Error("Invalid interval");
  const o = /* @__PURE__ */ new Map();
  for (const m of [...l].sort((g, r) => g.timestampMs - r.timestampMs)) {
    const g = Math.floor(m.timestampMs / (c * 6e4)) * c * 6e4, r = o.get(g);
    r ? (r.high = Math.max(r.high, m.high), r.low = Math.min(r.low, m.low), r.close = m.close, r.volume += m.volume) : o.set(g, { ...m, timestampMs: g });
  }
  return [...o.values()];
}
const b = (l) => l.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 }), E = (l) => new Date(l).toLocaleTimeString("en-US", { timeZone: "America/New_York", hour: "2-digit", minute: "2-digit", hour12: !1 }), B = (l) => l.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
function ae(l, c, o, m, g = "demo", r = [], t = {}) {
  const I = l.candles.slice(0, c), s = t.prepared ? I : ee(I, o);
  if (!s.length) return '<div class="live-empty">Sin velas para este intervalo.</div>';
  const $ = t.gex ? 1100 : 900, N = t.gex ? t.height ?? 560 : 350, p = t.gex ? 24 : 14, x = t.gex ? 90 : 78, M = t.gex ? 34 : 22, S = t.gex ? N - 130 : 270, k = t.gex ? 770 : $ - x, P = N - 33, T = m ? l.levels.filter((e) => Number.isFinite(e.price) && e.price > 0) : [], V = r.filter((e) => Number.isFinite(e.timestampMs) && (e.price === void 0 || Number.isFinite(e.price) && e.price > 0)).map((e) => ({ mark: e, index: s.findIndex((a) => e.timestampMs >= a.timestampMs && e.timestampMs < a.timestampMs + o * 6e4) })).filter((e) => e.index >= 0), _ = t.viewport ? s.filter((e, a) => a >= t.viewport.start - 1 && a <= t.viewport.start + t.viewport.count) : s, f = [...(_.length ? _ : s.slice(-1)).flatMap((e) => [e.high, e.low]), ...T.map((e) => e.price), ...(t.scalePrices ?? []).filter((e) => Number.isFinite(e) && e > 0)], Y = V.length ? (Math.max(...f) - Math.min(...f)) * 0.15 : 0, A = Math.max(Y, t.gex ? Math.max((Math.max(...f) - Math.min(...f)) * 0.06, Math.max(...f) * 1e-3) : 0.4), C = t.priceRange?.every(Number.isFinite) && t.priceRange[0] > 0 && t.priceRange[1] > t.priceRange[0] ? t.priceRange : void 0, v = C?.[0] ?? Math.min(...f) - A, u = C?.[1] ?? Math.max(...f) + A, i = (e) => M + (u - e) / (u - v) * (S - M), F = (k - p) / Math.max(t.viewport?.count ?? s.length, 1), d = (e) => p + (e - (t.viewport?.start ?? 0) + 0.5) * F, j = s[s.length - 1], H = Math.max(...s.map((e) => e.volume), 1), q = Array.from({ length: 6 }, (e, a) => {
    const n = v + (u - v) * a / 5;
    return `<line x1="0" x2="${$ - x}" y1="${i(n)}" y2="${i(n)}" class="grid-line"/><text x="${$ - x + 14}" y="${i(n) + 4}" class="axis">${b(n)}</text>`;
  }).join(""), z = s.map((e, a) => {
    if (d(a) < p - F || d(a) > k + F) return "";
    const n = (w) => Math.max(v, Math.min(u, w)), R = e.close >= e.open, h = R ? "var(--success)" : "var(--danger)", y = Math.max(2, Math.min(F * 0.56, 13));
    return `<g><title>${E(e.timestampMs)} ET · O ${b(e.open)} H ${b(e.high)} L ${b(e.low)} C ${b(e.close)} · Vol ${e.volume}${t.candleDetail ? ` · ${B(t.candleDetail(a))}` : ""}</title>
      ${e.high >= v && e.low <= u ? `<line x1="${d(a)}" x2="${d(a)}" y1="${i(n(e.high))}" y2="${i(n(e.low))}" stroke="${h}"/>` : ""}
      ${e.high >= v && e.low <= u ? `<rect x="${d(a) - y / 2}" y="${Math.min(i(n(e.open)), i(n(e.close)))}" width="${y}" height="${Math.max(1.7, Math.abs(i(n(e.open)) - i(n(e.close))))}" rx="1" fill="${h}"/>` : ""}
      <rect class="candle-volume" x="${d(a) - y / 2}" y="${P - e.volume / H * 29}" width="${y}" height="${e.volume / H * 29}" fill="${h}" opacity=".35"/></g>`;
  }).join(""), W = T.map((e) => {
    const a = e.kind === "magnet" ? "var(--mint)" : e.kind === "resistance" ? "var(--warning)" : "var(--success)";
    return `<line x1="0" x2="${$ - x}" y1="${i(e.price)}" y2="${i(e.price)}" stroke="${a}" stroke-opacity=".5" stroke-dasharray="4 5"/><text x="20" y="${i(e.price) - 8}" fill="${a}" class="level-label">${B(e.label.toUpperCase())}</text>`;
  }).join(""), Z = s.filter((e, a) => d(a) >= p && d(a) <= k && a % Math.max(1, Math.floor((t.viewport?.count ?? s.length) / 5)) === 0).map((e) => {
    const a = s.indexOf(e);
    return `<text x="${d(a)}" y="${N - 8}" text-anchor="middle" class="axis">${E(e.timestampMs)}</text>`;
  }).join(""), L = /* @__PURE__ */ new Map(), G = V.map(({ mark: e, index: a }) => {
    const n = `${a}:${e.kind}`, R = L.get(n) ?? 0;
    L.set(n, R + 1);
    const h = e.kind === "entry", y = s[a], w = d(a), D = i(h ? y.low : y.high) + (h ? 7 : -7) + (h ? 1 : -1) * R * 24, U = D + (h ? 10 : -10), X = D + (h ? 22 : -15), O = h ? "#20f5ad" : "#ffca4a";
    return `<g class="paper-chart-mark" data-mark-kind="${e.kind}" data-mark-source="${e.source ?? "model"}" data-mark-time="${e.timestampMs}"><title>${B(e.label)} · ${E(e.timestampMs)} ET</title><polygon points="${w},${D} ${w - 6},${U} ${w + 6},${U}" fill="${O}" stroke="#07132b" stroke-width="1"/><text x="${w}" y="${X}" text-anchor="middle" fill="${O}" font-size="10" font-weight="700">${h ? "E" : "S"}${e.source === "broker" ? "" : " SIM"}</text></g>`;
  }).join(""), J = t.overlay?.({ left: p, end: $ - x, candleEnd: k, top: M, floor: S, min: v, max: u, x: d, y: i }) ?? "", K = t.gex ? `preserveAspectRatio="none" tabindex="0" data-gex-svg data-view-start="${t.viewport?.start ?? 0}" data-step="${F}" data-left="${p}" data-end="${$ - x}" data-candle-end="${k}" data-top="${M}" data-floor="${S}" data-times="${s.map((e) => e.timestampMs).join(",")}" data-price-min="${v}" data-price-max="${u}"` : "", Q = t.gex ? `<line data-gex-cross-x pointer-events="none" visibility="hidden" x1="0" x2="0" y1="${M}" y2="${S}" stroke="#d1efff80" stroke-dasharray="3 3"/><line data-gex-cross-y pointer-events="none" visibility="hidden" x1="${p}" x2="${$ - x}" y1="0" y2="0" stroke="#d1efff80" stroke-dasharray="3 3"/>` : "";
  return `<svg class="market-chart" ${K} viewBox="0 0 ${$} ${N}" role="img" aria-label="${g === "demo" ? "Velas de demostración" : "Velas recibidas desde Data"} de ${B(l.symbol)}, intervalo ${o} minutos">
    ${q}${J}${W}<defs><clipPath id="candle-bounds"><rect x="${p}" y="${M}" width="${k - p}" height="${P - M}"/></clipPath></defs><g clip-path="url(#candle-bounds)">${z}${G}</g>${Q}<line x1="0" x2="${$ - x}" y1="${i(j.close)}" y2="${i(j.close)}" stroke="var(--mint)" stroke-opacity=".45" stroke-dasharray="2 4"/>
    <rect x="${$ - x + 3}" y="${i(j.close) - 11}" width="72" height="22" rx="4" fill="var(--mint)"/><text x="${$ - x + 39}" y="${i(j.close) + 4}" text-anchor="middle" class="last-price">${b(j.close)}</text>${Z}
  </svg>`;
}
function ie(l, c) {
  const o = l.candles.slice(Math.max(0, c - 30), c), m = Math.min(...o.map((r) => r.close)), g = Math.max(...o.map((r) => r.close));
  return `<svg viewBox="0 0 110 30" aria-hidden="true"><polyline points="${o.map((r, t) => `${t / Math.max(1, o.length - 1) * 110},${27 - (r.close - m) / (g - m || 1) * 24}`).join(" ")}" fill="none" stroke="currentColor" stroke-width="1.7"/></svg>`;
}
export {
  ae as chart,
  ie as sparkline
};
