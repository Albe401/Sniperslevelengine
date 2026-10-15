/*
 * Everything the panels show is derived here from one LivePayload (live or demo):
 * key levels with/without 0DTE, heuristic scores, alerts between two updates, and the text read.
 */
import type { DictKey } from "../i18n/dict";
import { CONV_SIGNS, type Convention, type ExpiryRow, type LivePayload } from "./types";
import { clamp, fmtLvl, fmtMoney, pct, unsigned } from "./format";

const TMY = 252 * 390;
const logistic = (x: number) => 1 / (1 + Math.exp(-x));

/** Strike exposures after applying the dealer convention: call ≥ 0 magnitude × sign, same for put. */
export interface StrikeView { K: number; call: number; put: number; }
export interface View {
  S: number; minute: number; L: number;
  strikes: StrikeView[];
  cw: number; pw: number; mg: number; flip: number | null; vt: number | null; prevClose: number | null;
  netG: number; grossG: number; ratio: number; flipPct: number; zShare: number;
  charmFlow: number; charmR: number; atmIv: number; mom: number;
  sc: { control: number; volx: number; pin: number; cwt: number; pwt: number; charm: number; bias: number };
}

export function priceAt(path: [number, number][], minute: number): number | null {
  let best: number | null = null;
  for (const [m, p] of path) { if (m <= minute + 1e-9) best = p; else break; }
  return best;
}

export function crossingNearest(prices: number[], vals: number[], S: number): number | null {
  let best: number | null = null, bd = Infinity;
  for (let j = 0; j < vals.length - 1; j++) {
    const a = vals[j], b = vals[j + 1];
    if ((a <= 0 && b > 0) || (a >= 0 && b < 0)) {
      const x = prices[j] + (prices[j + 1] - prices[j]) * (-a) / (b - a);
      if (Math.abs(x - S) < bd) { bd = Math.abs(x - S); best = x; }
    }
  }
  return best;
}

export function deriveView(p: LivePayload, zdte: boolean, conv: Convention): View {
  const S = p.market.spot, prof = p.profile, { c: sc, p: sp } = CONV_SIGNS[conv];
  const L = p.market.session_minutes || 390, minute = p.market.now_minute;
  const cK = zdte ? "c_all" : "c_ex0", pK = zdte ? "p_all" : "p_ex0";

  // Call/put gamma magnitudes by strike (dealer-long), then signed by the convention.
  const strikes: StrikeView[] = prof
    ? prof.strikes.map(s => ({ K: s.strike, call: sc * (zdte ? s.c : s.c - s.c0), put: sp * (zdte ? s.p : s.p - s.p0) }))
    : p.by_strike.map(s => ({ K: s.strike, call: sc * Math.abs(s.call_gex), put: sp * Math.abs(s.put_gex) }));
  if (!strikes.length) strikes.push({ K: S, call: 0, put: 0 });   // no usable strikes: keep panels stable

  // Walls = largest call / put gamma concentration (sign-independent); max gamma = largest |net|.
  let cw = strikes[0], pw = strikes[0], mg = strikes[0];
  for (const s of strikes) {
    if (Math.abs(s.call) > Math.abs(cw.call)) cw = s;
    if (Math.abs(s.put) > Math.abs(pw.put)) pw = s;
    if (Math.abs(s.call + s.put) > Math.abs(mg.call + mg.put)) mg = s;
  }

  let netG = 0, grossG = 0, flip: number | null = null, vt: number | null = null, charmFlow = 0, charmR = 0, zShare = 0;
  if (prof) {
    const g = prof.totals.gex, f = prof.totals.charm_flow;
    netG = sc * g[cK] + sp * g[pK];
    grossG = g[cK] + g[pK];
    const fp = prof.fine.prices, fg = prof.fine.gamma;
    flip = crossingNearest(fp, fg[cK].map((c, i) => sc * c + sp * fg[pK][i]), S);
    vt = crossingNearest(fp, fg.c_ex0.map((c, i) => sc * c + sp * fg.p_ex0[i]), S);
    charmFlow = sc * f[cK] + sp * f[pK];
    const gc = prof.totals.gross_charm[zdte ? "all" : "ex0"];
    charmR = gc ? charmFlow / gc : 0;
    const gAll = g.c_all + g.p_all;
    zShare = zdte && gAll ? (gAll - g.c_ex0 - g.p_ex0) / gAll : 0;
  } else {
    for (const s of strikes) { netG += s.call + s.put; grossG += Math.abs(s.call) + Math.abs(s.put); }
    flip = p.key_levels.zero_gamma ?? p.key_levels.gamma_flip;
    vt = p.key_levels.vol_trigger;
  }
  const ratio = grossG ? netG / grossG : 0;
  const flipPct = flip !== null ? (S - flip) / S * 100 : (ratio >= 0 ? 2 : -2);
  const atmIv = prof?.atm_iv ?? 0.2;
  const p30 = priceAt(p.path, minute - 30);
  const mom = p30 ? (S - p30) / S * 100 : 0;

  const sc_ = {
    control: 50 + 50 * ratio,
    volx: 100 * logistic(-2.5 * flipPct),
    pin: 100 * Math.exp(-Math.abs(S - mg.K) / (S * 0.0035)) * (ratio > 0 ? 1 : 0.5) * (0.35 + 0.65 * clamp(minute / L, 0, 1)),
    cwt: 100 * Math.exp(-Math.abs(cw.K - S) / (S * 0.005)),
    pwt: 100 * Math.exp(-Math.abs(pw.K - S) / (S * 0.005)),
    charm: 50 + 50 * charmR,
    bias: 50 + 50 * Math.tanh(1.8 * charmR + 0.5 * Math.tanh(flipPct) + 0.6 * Math.tanh(mom / 0.3)),
  };
  return { S, minute, L, strikes, cw: cw.K, pw: pw.K, mg: mg.K, flip, vt, prevClose: p.market.prev_close,
    netG, grossG, ratio, flipPct, zShare, charmFlow, charmR, atmIv, mom, sc: sc_ };
}

/* ---------- dominant strikes ---------- */
export type DomMetric = "gamma" | "oi" | "oi0" | "vol";
export interface DomRow { rank: number; K: number; c: number; p: number; total: number; share: number; side: "calls" | "puts"; sidePct: number; dist: number; }
export interface Dominance { metric: DomMetric; rows: DomRow[]; top5: number; total: number; strikes: number; available: boolean; }

/**
 * Where the book is concentrated: each strike's share of the total gamma $, open interest or
 * volume (calls + puts) inside the analysed window. Honors the 0DTE toggle. Sign-free, so it does
 * not depend on the dealer convention.
 */
export function dominance(p: LivePayload, zdte: boolean, metric: DomMetric, n = 8): Dominance {
  const S = p.market.spot;
  let raw: { K: number; c: number; p: number }[];
  if (p.profile) {
    raw = p.profile.strikes.map(s => {
      const pick = (a?: number, a0?: number) => Math.max(0, (a ?? 0) - (zdte ? 0 : (a0 ?? 0)));
      if (metric === "gamma") return { K: s.strike, c: pick(s.c, s.c0), p: pick(s.p, s.p0) };
      if (metric === "oi0") return { K: s.strike, c: Math.max(0, s.oi_c0 ?? 0), p: Math.max(0, s.oi_p0 ?? 0) };
      if (metric === "oi") return { K: s.strike, c: pick(s.oi_c, s.oi_c0), p: pick(s.oi_p, s.oi_p0) };
      return { K: s.strike, c: pick(s.vol_c, s.vol_c0), p: pick(s.vol_p, s.vol_p0) };
    });
  } else {
    raw = p.by_strike.map(s => metric === "gamma" ? { K: s.strike, c: Math.abs(s.call_gex), p: Math.abs(s.put_gex) }
      : metric === "oi" ? { K: s.strike, c: s.call_oi, p: s.put_oi } : { K: s.strike, c: 0, p: 0 });
  }
  const total = raw.reduce((a, r) => a + r.c + r.p, 0);
  const sorted = raw.filter(r => r.c + r.p > 0).sort((a, b) => (b.c + b.p) - (a.c + a.p));
  const rows: DomRow[] = sorted.slice(0, n).map((r, i) => {
    const tot = r.c + r.p;
    return { rank: i + 1, K: r.K, c: r.c, p: r.p, total: tot, share: total ? tot / total : 0,
      side: r.c >= r.p ? "calls" : "puts", sidePct: tot ? Math.max(r.c, r.p) / tot : 0, dist: (r.K - S) / S * 100 };
  });
  const top5 = rows.slice(0, 5).reduce((a, r) => a + r.share, 0);
  return { metric, rows, top5, total, strikes: raw.length, available: total > 0 };
}

/* ---------- gamma by expiration ---------- */
export interface ExpView {
  rows: (ExpiryRow & { net: number; gross: number; share: number; after: number; flips: boolean })[];
  total: number;                     // net gamma now (convention applied)
  firstFlip: number;                 // index of the first expiration after which the regime flips, −1 if none
  available: boolean;
}

/**
 * Which expirations carry the positioning. For each one: its share of gross gamma, its net gamma
 * under the convention, and the net gamma that remains once it (and all earlier ones) has expired.
 * A sign change there means the dealer regime flips after that date.
 */
export function expiryView(p: LivePayload, zdte: boolean, conv: Convention): ExpView {
  const ex = (p.profile?.expiries ?? []).filter(e => zdte || e.bdays > 0);
  const { c: sc, p: sp } = CONV_SIGNS[conv];
  const gross = ex.reduce((a, e) => a + e.c + e.p, 0);
  const total = ex.reduce((a, e) => a + sc * e.c + sp * e.p, 0);
  let cum = 0, firstFlip = -1;
  const rows = ex.map((e, i) => {
    const net = sc * e.c + sp * e.p;
    cum += net;
    const after = total - cum;
    const flips = i < ex.length - 1 && Math.sign(after) !== Math.sign(total) && Math.abs(after) > 0.02 * gross;
    if (flips && firstFlip < 0) firstFlip = i;
    return { ...e, net, gross: e.c + e.p, share: gross ? (e.c + e.p) / gross : 0, after, flips };
  });
  return { rows, total, firstFlip, available: rows.length > 0 };
}

/* ---------- volatility: expected move and vanna ---------- */
export interface VolView {
  available: boolean;
  day: { move: number; lo: number; hi: number; iv: number; minutes: number } | null;
  week: { move: number; lo: number; hi: number; iv: number; bdays: number } | null;
  vannaDown: number | null;          // $ dealers trade if implied vol FALLS 1 point (+ buy, − sell), convention applied
  cwIn: boolean | null; pwIn: boolean | null;   // walls inside today's 1σ range?
}

/**
 * Expected move = 1σ range implied by ATM volatility (≈68% of outcomes if implied vol is fair).
 * Vanna flow = what dealers must trade to stay delta-neutral if implied volatility falls 1 point:
 * positive = they buy (typical "vanna rally" when volatility is sold), negative = they sell.
 * If volatility RISES 1 point the flow is roughly the opposite.
 */
export function volView(p: LivePayload, v: View, zdte: boolean, conv: Convention): VolView {
  const S = v.S, em = p.profile?.em, vf = p.profile?.totals.vanna_flow, { c: sc, p: sp } = CONV_SIGNS[conv];
  const day = em?.day ? { move: em.day.move, lo: S - em.day.move, hi: S + em.day.move, iv: em.day.iv, minutes: em.day.minutes } : null;
  const week = em?.week ? { move: em.week.move, lo: S - em.week.move, hi: S + em.week.move, iv: em.week.iv, bdays: em.week.bdays } : null;
  const vannaDown = vf ? sc * vf[zdte ? "c_all" : "c_ex0"] + sp * vf[zdte ? "p_all" : "p_ex0"] : null;
  const inDay = (k: number) => (day ? k >= day.lo && k <= day.hi : null);
  return { available: !!(day || vf), day, week, vannaDown, cwIn: inDay(v.cw), pwIn: inDay(v.pw) };
}

/* ---------- liquidity magnet ---------- */
export interface MagnetRow { K: number; pull: number; share: number; net: number; dist: number; }
export interface Magnet {
  K: number | null;                  // most likely magnet strike
  strength: number;                  // 0–100 heuristic
  rows: MagnetRow[];                 // top candidates by pull
  maxPain: number | null; maxPainDays: number | null; maxPain0: boolean;
  confluence: boolean;               // max pain close to the magnet strike
  accel: number | null;              // strongest negative-gamma strike near price (price accelerates there)
  em: number;                        // expected move for the rest of the session ($ of the underlying)
  regimeLong: boolean;
  available: boolean;
}

/**
 * Liquidity magnet: where dealer hedging tends to pull price. Dealers who are long gamma at a strike
 * sell into rallies and buy dips around it, so price gets "pinned" there. Each strike's pull =
 * positive net gamma (convention applied) × a Gaussian weight on its distance to spot measured in
 * the expected move left today. The nearest expiration's max pain is shown as a second reference
 * and raises the strength when it agrees. Negative-gamma strikes do the opposite (acceleration).
 */
export function magnet(p: LivePayload, v: View, n = 4, includeZero = true): Magnet {
  const S = v.S, prof = p.profile;
  const rem = Math.max(45, v.L - v.minute) + (v.minute >= v.L ? 390 : 0);   // after the close: next session
  const em = S * Math.max(0.05, v.atmIv) * Math.sqrt(rem / TMY);
  const lo = prof ? prof.prices[0] : S * 0.97, hi = prof ? prof.prices[prof.prices.length - 1] : S * 1.03;
  const w = (K: number) => Math.exp(-0.5 * Math.pow((K - S) / (1.5 * em), 2));
  let total = 0, accel: number | null = null, accelV = 0;
  const cand: MagnetRow[] = [];
  for (const s of v.strikes) {
    if (s.K < lo || s.K > hi) continue;
    const net = s.call + s.put, ww = w(s.K);
    if (net > 0) { const pull = net * ww; total += pull; cand.push({ K: s.K, pull, share: 0, net, dist: (s.K - S) / S * 100 }); }
    else if (net * ww < accelV) { accelV = net * ww; accel = s.K; }
  }
  cand.sort((a, b) => b.pull - a.pull);
  const rows = cand.slice(0, n).map(r => ({ ...r, share: total ? r.pull / total : 0 }));
  const mp = !includeZero && prof?.max_pain?.is_0dte ? null : prof?.max_pain ?? null;
  const top = rows[0];
  const confluence = !!(top && mp && Math.abs(mp.strike - top.K) <= Math.max(0.6 * em, S * 0.0015));
  const regimeLong = v.ratio > 0;
  const tod = 0.6 + 0.4 * clamp(v.minute / v.L, 0, 1);
  const strength = top ? 100 * clamp((0.35 + 0.65 * top.share) * (regimeLong ? 1 : 0.55) * tod + (confluence ? 0.12 : 0), 0, 1) : 0;
  return { K: top ? top.K : null, strength, rows, maxPain: mp ? mp.strike : null, maxPainDays: mp ? mp.bdays : null,
    maxPain0: !!mp?.is_0dte, confluence, accel, em, regimeLong, available: !!top };
}

/* ---------- estimated dealer positioning (contracts) ---------- */
export interface DealerRow { K: number; calls: number; puts: number; net: number; delta: number; dist: number; }
export interface DealerBook {
  rows: DealerRow[];                 // strikes with the most open interest, sorted by strike
  callsLong: number; callsShort: number; putsLong: number; putsShort: number; netContracts: number;
  delta: number;                     // dealer option delta, in contracts of delta (×100 = underlying units)
  hedgeUnits: number;                // underlying units dealers hold to be delta-neutral (= −delta×100); futures: units of the proxy
  hedgeNotional: number;             // $ of that hedge
  unitsOf?: string;                  // futures: the proxy whose units the hedge is expressed in
  lo: number; hi: number;            // strike window used for the totals (the map's price range)
  available: boolean;
}

/**
 * Estimated dealer position from open interest and the chosen convention. Real dealer inventories
 * are not public; this assumes dealers are the counterparty of all open interest with the signs of
 * the convention (standard: long calls, short puts).
 */
export function dealerBook(p: LivePayload, zdte: boolean, conv: Convention, n = 12): DealerBook {
  const S = p.market.spot, { c: sc, p: sp } = CONV_SIGNS[conv];
  const empty: DealerBook = { rows: [], callsLong: 0, callsShort: 0, putsLong: 0, putsShort: 0, netContracts: 0, delta: 0, hedgeUnits: 0, hedgeNotional: 0, lo: 0, hi: 0, available: false };
  if (!p.profile || p.profile.strikes[0]?.oi_c === undefined) return empty;
  const x = (a?: number, a0?: number) => (a ?? 0) - (zdte ? 0 : (a0 ?? 0));
  // Totals over the map's price range: near-the-money positioning is what drives hedging flows.
  const lo = p.profile.prices[0], hi = p.profile.prices[p.profile.prices.length - 1];
  let cOI = 0, pOI = 0, dlt = 0;
  const all = p.profile.strikes.filter(s => s.strike >= lo && s.strike <= hi).map(s => {
    const oc = x(s.oi_c, s.oi_c0), op = x(s.oi_p, s.oi_p0);
    const d = sc * x(s.dl_c, s.dl_c0) + sp * x(s.dl_p, s.dl_p0);
    cOI += oc; pOI += op; dlt += d;
    return { K: s.strike, calls: sc * oc, puts: sp * op, net: sc * oc + sp * op, delta: d, dist: (s.strike - S) / S * 100, oi: oc + op };
  });
  const rows = all.filter(r => r.oi > 0).sort((a, b) => b.oi - a.oi).slice(0, n).sort((a, b) => b.K - a.K)
    .map(({ K, calls, puts, net, delta, dist }) => ({ K, calls, puts, net, delta, dist }));
  const cl = sc > 0 ? cOI : 0, cs = sc < 0 ? cOI : 0, pl = sp > 0 ? pOI : 0, ps = sp < 0 ? pOI : 0;
  return { rows, callsLong: cl, callsShort: cs, putsLong: pl, putsShort: ps, netContracts: cl - cs + pl - ps,
    delta: dlt, hedgeUnits: -dlt * 100, hedgeNotional: -dlt * 100 * (p.asset?.proxy_spot ?? S), unitsOf: p.asset?.class === "future" ? p.asset.proxy : undefined, lo, hi, available: cOI + pOI > 0 };
}

/* ---------- alerts ---------- */
export interface AlertEvent { id: string; key: string; at: number; minute: number; grade: string; tk: DictKey; dk: DictKey; pr: Record<string, string | number>; color: string; }
export const ACOL = { violet: "#8b5cff", violet2: "#b79bff", red: "#ff3d6e", cyan: "#3ee6ff", green: "#35f2a1", amber: "#ffc24b" };

/**
 * Compare two consecutive views and return new alerts. `last` holds when each alert key
 * last fired (session minute) so the same alert is not repeated within 20 minutes.
 */
export function detectAlerts(prev: View | null, next: View, path: [number, number][], zdte: boolean, last: Record<string, number>): AlertEvent[] {
  if (!prev) return [];
  const out: AlertEvent[] = [];
  const m = next.minute;
  const fire = (key: string, grade: string, tk: DictKey, dk: DictKey, pr: Record<string, string | number>, color: string) => {
    if (last[key] !== undefined && m - last[key] < 20 && m >= last[key]) return;
    last[key] = m;
    out.push({ id: key + ":" + Date.now() + ":" + Math.random().toString(36).slice(2, 6), key, at: Date.now(), minute: m, grade, tk, dk, pr, color });
  };
  if (next.flip !== null && prev.flip !== null) {
    const a = prev.S - prev.flip, b = next.S - next.flip;
    if (a > 0 && b <= 0) fire("breach", "A+", "aBreachT", "aBreachD", { v: fmtLvl(next.flip) }, ACOL.red);
    if (a < 0 && b >= 0) fire("reclaim", "A", "aReclaimT", "aReclaimD", { v: fmtLvl(next.flip) }, ACOL.green);
  }
  if (Math.abs(next.S - next.cw) / next.S < 0.0015) fire("cw", "A", "aCwT", "aCwD", { v: fmtLvl(next.cw) }, ACOL.violet2);
  if (Math.abs(next.S - next.pw) / next.S < 0.0015) fire("pw", "A", "aPwT", "aPwD", { v: fmtLvl(next.pw) }, ACOL.cyan);
  if (next.sc.pin > 72 && m >= next.L - 90) fire("pin", "B+", "aPinT", "aPinD", { v: fmtLvl(next.mg) }, ACOL.amber);
  const s5 = priceAt(path, m - 5);
  if (s5) {
    const mv = Math.abs(next.S - s5) / s5, sig5 = next.atmIv * Math.sqrt(5 / TMY);
    if (next.ratio < 0 && mv > 2 * sig5) fire("accel", "A+", "aAccelT", "aAccelD", { v: (mv * 100).toFixed(2) }, ACOL.red);
    if (next.ratio > 0 && prev.ratio > 0 && m >= 60 && mv > 2.2 * sig5)
      fire("squeeze", "A", "aSqT", "aSqD", { v: (mv * 100).toFixed(2), w: next.S > s5 ? "@toCW" : "@toPW" }, ACOL.violet);
  }
  if (zdte && m >= next.L - 60 && next.zShare > 0.3) fire("unwind", "B", "aUnwT", "aUnwD", { v: (next.zShare * 100).toFixed(0) }, ACOL.amber);
  return out;
}

/* ---------- NeuroCore read ---------- */
type T = (k: DictKey, p?: Record<string, string | number>) => string;
const esc = (x: string) => x.replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]!));

export function aiRead(t: T, v: View, symRaw: string, zdte: boolean, dom?: DomRow, mag?: Magnet, vol?: VolView, exv?: ExpView): string {
  const sym = esc(symRaw);                        // the text is rendered as HTML
  const pos = v.netG >= 0, side = pos ? t("sideL") : t("sideS");
  let s = v.flip === null
    ? t("aiNoFlip", { side })
    : t("aiFlip", { sym, px: fmtLvl(v.S), d: Math.abs(v.flipPct).toFixed(2), dir: v.flipPct >= 0 ? t("above") : t("below"), flip: fmtLvl(v.flip) });
  s += t("aiDealers", { side, g: unsigned(fmtMoney(v.netG)), eff: pos ? t("effDamp") : t("effAmp") });
  s += t("aiWalls", { cw: fmtLvl(v.cw), cwp: pct(v.cw, v.S), role: pos ? t("roleRes") : t("roleMag"), pw: fmtLvl(v.pw), pwp: pct(v.pw, v.S) });
  if (dom) s += t("aiDom", { k: fmtLvl(dom.K), s: (dom.share * 100).toFixed(1), side: dom.side === "calls" ? t("sideCalls") : t("sidePuts"), d: pct(dom.K, v.S) });
  if (mag && mag.K !== null) s += t("aiMag", { k: fmtLvl(mag.K), d: pct(mag.K, v.S), s: mag.strength.toFixed(0), mp: mag.maxPain !== null ? t("aiMagMp", { mp: fmtLvl(mag.maxPain) }) : "" });
  if (exv?.available && exv.rows[0]) {
    const r0 = exv.rows[0];
    if (r0.share >= 0.15) s += t("aiExpNear", { d: r0.expiration.slice(5), s: (r0.share * 100).toFixed(0) });
    if (exv.firstFlip >= 0) { const f = exv.rows[exv.firstFlip]; s += t("aiExpFlip", { d: f.expiration.slice(5), a: fmtMoney(f.after) }); }
  }
  if (vol?.day) s += t("aiEm", { m: fmtLvl(vol.day.move), lo: fmtLvl(vol.day.lo), hi: fmtLvl(vol.day.hi) });
  if (vol?.vannaDown) s += t("aiVanna", { act: vol.vannaDown >= 0 ? t("buying") : t("selling"), amt: unsigned(fmtMoney(Math.abs(vol.vannaDown))) });
  if (v.charmFlow) s += t("aiCharm", { act: v.charmFlow >= 0 ? t("buying") : t("selling"), amt: unsigned(fmtMoney(Math.abs(v.charmFlow))) });
  if (v.minute >= v.L - 90 && v.sc.pin > 55) s += t("aiPin", { mg: fmtLvl(v.mg) });
  if (zdte && v.zShare > 0.3) s += t("aiZ", { z: (v.zShare * 100).toFixed(0) });
  return s;
}
