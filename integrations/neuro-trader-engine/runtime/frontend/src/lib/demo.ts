/*
 * Demo engine — used ONLY when the backend is unreachable or demo mode is forced.
 * Builds a synthetic option chain and runs the same Black-Scholes GEX / delta / charm math
 * as backend/gex_engine/profile.py, returning the exact LivePayload shape the backend sends.
 * Everything it returns is marked timeframe = "DEMO" and spot_source = "demo".
 */
import type { AssetInfo, LivePayload, Mode, PackedGrid, Part, Parts, Profile, ProfileStrike } from "./types";

const SESSION = 390, TMY = 252 * 390, MIN_T = 10 / TMY, R = 0.04, INV = 0.3989422804014327;
const N_P = 72, N_T = 53;

interface SymCfg { close: number; step: number; round: number; iv: number; range: number; oi: number; gap: number; }
export const DEMO_SYMS: Record<string, SymCfg> = {
  SPX:  { close: 7485.0, step: 10,  round: 50, iv: 0.14, range: 0.03,  oi: 9000,  gap: 0.0018 },
  SPY:  { close: 747.20, step: 1,   round: 5,  iv: 0.14, range: 0.03,  oi: 60000, gap: 0.0018 },
  QQQ:  { close: 611.40, step: 1,   round: 5,  iv: 0.18, range: 0.035, oi: 42000, gap: 0.0022 },
  NVDA: { close: 182.30, step: 2.5, round: 10, iv: 0.42, range: 0.065, oi: 90000, gap: 0.004 },
  TSLA: { close: 341.80, step: 5,   round: 25, iv: 0.55, range: 0.075, oi: 52000, gap: 0.005 },
  AAPL: { close: 236.10, step: 2.5, round: 10, iv: 0.24, range: 0.042, oi: 60000, gap: 0.0025 },
  NDX:  { close: 25480,  step: 25,  round: 250, iv: 0.18, range: 0.035, oi: 2500, gap: 0.0022 },
  RUT:  { close: 2452,   step: 5,   round: 50, iv: 0.21, range: 0.04,  oi: 6000,  gap: 0.0025 },
};
const CLASS: Record<string, AssetInfo["class"]> = { SPX: "index", NDX: "index", RUT: "index", SPY: "etf", QQQ: "etf", NVDA: "stock", TSLA: "stock", AAPL: "stock" };
const DIV_Q: Record<string, number> = { SPX: 0.0125, NDX: 0.007, RUT: 0.013, SPY: 0.012, QQQ: 0.006, AAPL: 0.0045 };
/** Demo futures: index futures priced at fair value from their index (same as the backend without a futures plan). */
export const DEMO_FUTURES: Record<string, { proxy: string; es: string; en: string }> = {
  ES: { proxy: "SPX", es: "E-mini S&P 500", en: "E-mini S&P 500" }, MES: { proxy: "SPX", es: "Micro E-mini S&P 500", en: "Micro E-mini S&P 500" },
  NQ: { proxy: "NDX", es: "E-mini Nasdaq-100", en: "E-mini Nasdaq-100" }, MNQ: { proxy: "NDX", es: "Micro E-mini Nasdaq-100", en: "Micro E-mini Nasdaq-100" },
  RTY: { proxy: "RUT", es: "E-mini Russell 2000", en: "E-mini Russell 2000" }, M2K: { proxy: "RUT", es: "Micro E-mini Russell 2000", en: "Micro E-mini Russell 2000" },
};
export const isDemoSymbol = (s: string) => !!DEMO_SYMS[s] || (s.startsWith("/") && !!DEMO_FUTURES[s.slice(1)]);

interface Contract { K: number; days: number; z: boolean; call: number; put: number; net: number; iv: number; }

function ncdf(x: number): number {
  const t = 1 / (1 + 0.2316419 * Math.abs(x));
  const y = t * (0.319381530 + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))));
  const p = 1 - INV * Math.exp(-0.5 * x * x) * y;
  return x >= 0 ? p : 1 - p;
}
function mulberry32(a: number) {
  return () => { a = (a + 0x6D2B79F5) | 0; let t = Math.imul(a ^ (a >>> 15), 1 | a); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
}
function hashStr(s: string) { let h = 2166136261; for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 16777619); } return h >>> 0; }
function gauss(r: () => number) { const u = 1 - r(), v = r(); return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v); }
const tYears = (days: number, minute: number) => Math.max((days * SESSION + (SESSION - minute)) / TMY, MIN_T);
const isMult = (K: number, r: number) => { const q = K / r; return Math.abs(q - Math.round(q)) < 1e-6; };

function buildChain(sym: string, c: SymCfg): Contract[] {
  const rng = mulberry32(hashStr(sym + ":chain"));
  const S0 = c.close, lo = S0 * (1 - c.range * 2), hi = S0 * (1 + c.range * 2);
  const EXP = [{ days: 0, w: 0.28, width: 0.45 }, { days: 4, w: 0.4, width: 0.8 }, { days: 17, w: 0.75, width: 1.25 }];
  const cwK = Math.round(S0 * (1 + c.range * 0.4) / c.round) * c.round;
  const pwK = Math.round(S0 * (1 - c.range * 0.5) / c.round) * c.round;
  const out: Contract[] = [];
  for (let k = Math.ceil(lo / c.step); k * c.step <= hi; k++) {
    const K = Math.round(k * c.step * 100) / 100, m = K / S0 - 1;
    let bonus = 1;
    if (isMult(K, c.round)) bonus = 2.1;
    if (isMult(K, c.round * 2)) bonus = 3;
    for (const X of EXP) {
      const wd = c.range * X.width;
      const cz = (m - 0.12 * c.range) / wd, pz = (m + 0.5 * c.range) / (wd * 1.15);
      let call = c.oi * X.w * Math.exp(-cz * cz) * bonus * (0.55 + 0.9 * rng());
      let put = c.oi * X.w * 0.9 * Math.exp(-pz * pz) * bonus * (0.55 + 0.9 * rng());
      if (X.days > 0 && Math.abs(K - cwK) < 1e-6) call *= 4.5;
      if (X.days > 0 && Math.abs(K - pwK) < 1e-6) put *= 4.5;
      call = Math.round(call); put = Math.round(put);
      const iv = Math.max(c.iv * 0.7, c.iv * (1 - 1.8 * m)) * (X.days === 0 ? 1.1 : 1);
      out.push({ K, days: X.days, z: X.days === 0, call, put, net: call - put, iv });
    }
  }
  return out;
}

function buildPath(sym: string, c: SymCfg): number[] {
  const r = mulberry32(hashStr(sym + ":path"));
  let S = c.close * (1 + (r() * 2 - 1) * c.gap);
  const sig = c.iv * 1.05 / Math.sqrt(TMY), lo = c.close * (1 - c.range * 0.92), hi = c.close * (1 + c.range * 0.92);
  const p = [S];
  for (let m = 1; m <= SESSION; m++) {
    S = S * Math.exp(sig * gauss(r) - 0.5 * sig * sig);
    if (S > hi) S = hi - (S - hi);
    if (S < lo) S = lo + (lo - S);
    p.push(S);
  }
  return p;
}

type Acc = Record<Mode, Parts<number>>;
const PARTS: Part[] = ["c_all", "c_ex0", "p_all", "p_ex0"];
const zeroAcc = (): Acc => ({
  gamma: { c_all: 0, c_ex0: 0, p_all: 0, p_ex0: 0 }, delta: { c_all: 0, c_ex0: 0, p_all: 0, p_ex0: 0 },
  charm: { c_all: 0, c_ex0: 0, p_all: 0, p_ex0: 0 }, vanna: { c_all: 0, c_ex0: 0, p_all: 0, p_ex0: 0 },
});

/** Dealer-LONG exposures (+OI) of calls and puts separately at price S and minute (convention applied later). */
function bookAt(chain: Contract[], S: number, minute: number, ref: number): Acc {
  const a = zeroAcc();
  for (const c of chain) {
    const T = tYears(c.days, minute), v = c.iv * Math.sqrt(T);
    const d1 = (Math.log(S / c.K) + (R + 0.5 * c.iv * c.iv) * T) / v;
    const dr = (Math.log(ref / c.K) + (R + 0.5 * c.iv * c.iv) * T) / v;
    const pdf = INV * Math.exp(-0.5 * d1 * d1);
    const g = (pdf / v) * S;                                          // Γ·100·S²·1% per contract of OI
    const d = -(ncdf(d1) - ncdf(dr)) * 100 * S;
    const ch = -(-pdf * (2 * R * T - (d1 - v) * v) / (2 * T * v)) / 252 * 100 * S;
    const va = -pdf * (d1 - v) / c.iv * 0.01 * 100 * S;              // hedge $ if IV falls 1 point
    const add = (k: Part, oi: number) => { a.gamma[k] += oi * g; a.delta[k] += oi * d; a.charm[k] += oi * ch; a.vanna[k] += oi * va; };
    add("c_all", c.call); add("p_all", c.put);
    if (!c.z) { add("c_ex0", c.call); add("p_ex0", c.put); }
  }
  return a;
}

function pack(g: number[][]): PackedGrid {
  let mx = 0;
  for (const row of g) for (const x of row) mx = Math.max(mx, Math.abs(x));
  const unit = mx / 10000 || 1;
  return { unit, q: g.map(row => row.map(x => Math.round(x / unit))) };
}

interface Cached { cfg: SymCfg; chain: Contract[]; path: number[]; prices: number[]; minutes: number[]; grids: Profile["grids"]; }
const CACHE = new Map<string, Cached>();

function prepare(sym: string): Cached {
  const hit = CACHE.get(sym);
  if (hit) return hit;
  const cfg = DEMO_SYMS[sym] ?? DEMO_SYMS.SPY;
  const chain = buildChain(sym, cfg), path = buildPath(sym, cfg);
  const pmin = cfg.close * (1 - cfg.range), pmax = cfg.close * (1 + cfg.range);
  const prices = Array.from({ length: N_P }, (_, i) => pmin + (pmax - pmin) * i / (N_P - 1));
  const minutes = Array.from({ length: N_T }, (_, j) => SESSION * j / (N_T - 1));
  const raw = {} as Record<Mode, Parts<number[][]>>;
  (["gamma", "delta", "charm", "vanna"] as Mode[]).forEach(md => { raw[md] = { c_all: [], c_ex0: [], p_all: [], p_ex0: [] }; });
  for (const P of prices) {
    const rows = {} as Record<Mode, Parts<number[]>>;
    (["gamma", "delta", "charm", "vanna"] as Mode[]).forEach(md => { rows[md] = { c_all: [], c_ex0: [], p_all: [], p_ex0: [] }; });
    minutes.forEach((m, j) => {
      const a = bookAt(chain, P, Math.min(m, 386), cfg.close);
      (["gamma", "delta", "charm", "vanna"] as Mode[]).forEach(md => PARTS.forEach(k => { rows[md][k][j] = a[md][k]; }));
    });
    (["gamma", "delta", "charm", "vanna"] as Mode[]).forEach(md => PARTS.forEach(k => raw[md][k].push(rows[md][k])));
  }
  const grids = {} as Profile["grids"];
  (["gamma", "delta", "charm", "vanna"] as Mode[]).forEach(md => {
    grids[md] = { c_all: pack(raw[md].c_all), c_ex0: pack(raw[md].c_ex0), p_all: pack(raw[md].p_all), p_ex0: pack(raw[md].p_ex0) };
  });
  const out = { cfg, chain, path, prices, minutes, grids };
  CACHE.set(sym, out);
  return out;
}

export function demoPayload(symbol: string, minute: number): LivePayload {
  if (symbol.startsWith("/") && DEMO_FUTURES[symbol.slice(1)]) return demoFuture(symbol.slice(1), minute);
  const sym = DEMO_SYMS[symbol] ? symbol : "SPY";
  const P = prepare(sym), c = P.cfg, m = Math.max(0, Math.min(SESSION, Math.round(minute)));
  const S = P.path[m];

  const byK = new Map<number, ProfileStrike>();
  const gex: Parts<number> = { c_all: 0, c_ex0: 0, p_all: 0, p_ex0: 0 }, flow: Parts<number> = { c_all: 0, c_ex0: 0, p_all: 0, p_ex0: 0 };
  const vflow: Parts<number> = { c_all: 0, c_ex0: 0, p_all: 0, p_ex0: 0 };
  const byExp = new Map<number, { c: number; p: number; oi_c: number; oi_p: number }>();
  let gcAll = 0, gcEx = 0, atmIv = c.iv, atmD = Infinity, iv0 = c.iv, d0 = Infinity, iv4 = c.iv;
  for (const x of P.chain) {
    if (x.days === 0 && Math.abs(x.K - S) < d0) { d0 = Math.abs(x.K - S); iv0 = x.iv; }
    if (x.days === 4 && Math.abs(x.K - S) < atmD) { atmD = Math.abs(x.K - S); atmIv = x.iv; iv4 = x.iv; }
    const T = tYears(x.days, m), v = x.iv * Math.sqrt(T);
    const d1 = (Math.log(S / x.K) + (R + 0.5 * x.iv * x.iv) * T) / v, pdf = INV * Math.exp(-0.5 * d1 * d1);
    const g1 = (pdf / v) * S, cg = x.call * g1, pg = x.put * g1;
    const f1 = -(-pdf * (2 * R * T - (d1 - v) * v) / (2 * T * v)) / 252 * 100 * S, cf = x.call * f1, pf = x.put * f1;
    const va = -pdf * (d1 - v) / x.iv * 0.01 * 100 * S;
    vflow.c_all += x.call * va; vflow.p_all += x.put * va;
    { const b = byExp.get(x.days) ?? { c: 0, p: 0, oi_c: 0, oi_p: 0 };
      b.c += cg; b.p += pg; b.oi_c += x.call; b.oi_p += x.put; byExp.set(x.days, b); }
    if (!x.z) { vflow.c_ex0 += x.call * va; vflow.p_ex0 += x.put * va; }
    const e = byK.get(x.K) ?? { strike: x.K, c: 0, p: 0, c0: 0, p0: 0, oi_c: 0, oi_p: 0, oi_c0: 0, oi_p0: 0, vol_c: 0, vol_p: 0, vol_c0: 0, vol_p0: 0, dl_c: 0, dl_p: 0, dl_c0: 0, dl_p0: 0 };
    const dC = ncdf(d1), dlc = x.call * dC, dlp = x.put * (dC - 1);   // Σ OI·Δ (q≈0 in the demo)
    // sample volume: part of OI, higher at the money and on 0DTE (deterministic per strike)
    const turn = (0.05 + 0.4 * Math.abs(Math.sin(x.K * 12.9898 + x.days))) * (x.z ? 4 : 1) * (Math.abs(x.K / S - 1) < 0.01 ? 2 : 1);
    const vc = Math.round(x.call * turn), vp = Math.round(x.put * turn);
    e.dl_c! += dlc; e.dl_p! += dlp; if (x.z) { e.dl_c0! += dlc; e.dl_p0! += dlp; }
    e.c += cg; e.p += pg; e.oi_c! += x.call; e.oi_p! += x.put; e.vol_c! += vc; e.vol_p! += vp;
    if (x.z) { e.c0 += cg; e.p0 += pg; e.oi_c0! += x.call; e.oi_p0! += x.put; e.vol_c0! += vc; e.vol_p0! += vp; }
    byK.set(x.K, e);
    gex.c_all += cg; gex.p_all += pg; flow.c_all += cf; flow.p_all += pf; gcAll += Math.abs(cf) + Math.abs(pf);
    if (!x.z) { gex.c_ex0 += cg; gex.p_ex0 += pg; flow.c_ex0 += cf; flow.p_ex0 += pf; gcEx += Math.abs(cf) + Math.abs(pf); }
  }
  const strikes = [...byK.values()].sort((a, b) => a.strike - b.strike);
  const fineP = Array.from({ length: 121 }, (_, i) => P.prices[0] + (P.prices[N_P - 1] - P.prices[0]) * i / 120);
  const fa = fineP.map(p => bookAt(P.chain, p, m, c.close).gamma);
  const fine = { prices: fineP, gamma: { c_all: fa.map(a => a.c_all), c_ex0: fa.map(a => a.c_ex0), p_all: fa.map(a => a.p_all), p_ex0: fa.map(a => a.p_ex0) } };

  let cw = strikes[0], pw = strikes[0];
  for (const s of strikes) { if (s.c > cw.c) cw = s; if (s.p > pw.p) pw = s; }
  // max pain of the front (0DTE) expiration
  let maxPain: Profile["max_pain"] = null;
  { const fr = P.chain.filter(x => x.days === 0); let best = Infinity;
    for (const k of fr) {
      let pain = 0; for (const x of fr) pain += x.call * Math.max(0, k.K - x.K) + x.put * Math.max(0, x.K - k.K);
      if (pain < best) { best = pain; maxPain = { strike: k.K, bdays: 0, is_0dte: true, oi: fr.reduce((a, x) => a + x.call + x.put, 0) }; }
    } }
  const netStd = gex.c_all - gex.p_all, grossAll = gex.c_all + gex.p_all;

  const profile: Profile = {
    prices: P.prices, minutes: P.minutes, grids: P.grids, fine, strikes,
    totals: { gex, charm_flow: flow, gross_charm: { all: gcAll, ex0: gcEx }, vanna_flow: vflow },
    expiries: [...byExp.entries()].sort((a, b) => a[0] - b[0]).map(([d, b]) => {
      const dt = new Date(); let k = d;                    // business days ahead → calendar date
      while (k > 0) { dt.setDate(dt.getDate() + 1); if (dt.getDay() % 6 !== 0) k--; }
      const kind = d === 0 ? "0dte" : d >= 15 ? "monthly" : dt.getDay() === 5 ? "weekly" : "daily";
      return { expiration: dt.toISOString().slice(0, 10), bdays: d, kind: kind as "0dte", am: false, ...b };
    }),
    em: {
      day: { move: S * iv0 * Math.sqrt(Math.max(5, SESSION - m) / TMY), iv: iv0, minutes: Math.max(5, SESSION - m) },
      week: { move: S * iv4 * Math.sqrt((4 * SESSION + Math.max(5, SESSION - m)) / TMY), iv: iv4, bdays: 4 },
    },
    atm_iv: atmIv, ref_price: c.close, q: DIV_Q[sym] ?? 0, range_pct: c.range, contracts_used: P.chain.length * 2, has_0dte: true,
    max_pain: maxPain,
  };
  const cls = CLASS[sym] ?? "stock";
  const today = new Date().toISOString().slice(0, 10);
  return {
    symbol: sym,
    asset: { class: cls, symbol: sym, exercise: cls === "index" ? "european" : "american", dividend_yield: DIV_Q[sym] ?? 0, dividend_source: "demo" },
    key_levels: {
      underlying_price: S, call_wall: cw.strike, put_wall: pw.strike, gamma_flip: S, zero_gamma: null, vol_trigger: null,
      high_vol_level: S * atmIv / Math.sqrt(252), total_call_gex: gex.c_all, total_put_gex: -gex.p_all, net_gex: netStd, total_dex: 0,
      gamma_regime: netStd >= 0 ? "positive_gamma" : "negative_gamma", regime_confidence: grossAll ? Math.abs(netStd) / grossAll : 0,
      computation_ts: new Date().toISOString(), contracts_processed: P.chain.length * 2, expirations_used: ["DEMO"],
    },
    by_strike: strikes.map(s => ({ strike: s.strike, call_gex: s.c, put_gex: -s.p, net_gex: s.c - s.p, call_oi: 0, put_oi: 0 })),
    computation_ms: 0,
    market: {
      spot: S, spot_source: "demo", prev_close: c.close, vix: null, timeframe: "DEMO",
      session_date: today, session_minutes: SESSION, now_minute: m, session_state: "open",
      oi_date: "—", generated_at: new Date().toISOString(),
    },
    path: P.path.slice(0, m + 1).map((p, i) => [i, p] as [number, number]),
    path_source: "demo",
    profile,
  };
}

/** Index future in demo: same translation as the backend (additive basis = fair value − index). */
function demoFuture(root: string, minute: number): LivePayload {
  const f = DEMO_FUTURES[root], base = demoPayload(f.proxy, minute), q = DIV_Q[f.proxy] ?? 0;
  const S = base.market.spot, now = new Date();
  // front quarterly expiry (3rd Friday of Mar/Jun/Sep/Dec), rolled 8 days early
  let exp = new Date(now);
  for (let k = 0; k < 15; k++) {
    const d = new Date(now.getFullYear(), now.getMonth() + k, 15);
    if ((d.getMonth() + 1) % 3) continue;
    d.setDate(15 + ((5 - d.getDay() + 7) % 7));
    if (now.getTime() <= d.getTime() - 8 * 864e5) { exp = d; break; }
  }
  const T = Math.max(0, (exp.getTime() - now.getTime()) / (365 * 864e5)), F = S * Math.exp((R - q) * T), basis = F - S;
  const fn = (p: number) => p + basis;
  const pr = base.profile!;
  const out: LivePayload = {
    ...base,
    symbol: "/" + root,
    market: { ...base.market, spot: F, prev_close: base.market.prev_close === null ? null : fn(base.market.prev_close) },
    path: base.path.map(([m, p]) => [m, fn(p)] as [number, number]),
    by_strike: base.by_strike.map(r => ({ ...r, strike: fn(r.strike) })),
    key_levels: { ...base.key_levels, underlying_price: F, call_wall: fn(base.key_levels.call_wall), put_wall: fn(base.key_levels.put_wall), gamma_flip: fn(base.key_levels.gamma_flip) },
    profile: { ...pr, prices: pr.prices.map(fn), fine: { ...pr.fine, prices: pr.fine.prices.map(fn) }, strikes: pr.strikes.map(s => ({ ...s, strike: fn(s.strike) })), ref_price: fn(pr.ref_price),
      max_pain: pr.max_pain ? { ...pr.max_pain, strike: fn(pr.max_pain.strike) } : null },
    asset: { ...base.asset, class: "future", symbol: "/" + root, root, name_es: f.es, name_en: f.en, proxy: f.proxy, method: "additive", tier: "high",
      contract: root + "HMUZ"[Math.floor(exp.getMonth() / 3)] + String(exp.getFullYear() % 10), settlement_date: exp.toISOString().slice(0, 10),
      price_source: "fair_value_model", proxy_spot: S, basis },
  };
  return out;
}
