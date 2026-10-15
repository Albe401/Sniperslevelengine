/**
 * oi-heatmap-v5 CORE: pure calculation, no DOM, no network, no trading decisions.
 * UI lives elsewhere (see INTERFAZ.md); it must render these outputs, never recompute them.
 * Daily open interest, never execution-side inference. Based on v3 (2026-10-14) with
 * cell-local completeness: a verified cell keeps its exact OI when other rows have problems.
 *
 * Formula (measurement): OI(cell) = sum of open_interest over reference tickers with the
 * same (underlying, strike, expiration, side) AND standard deliverable. Unit: contracts.
 *   Source: OIC "open option contracts still active", compiled by OCC from opening/closing
 *   trades; Massive open_interest = "held at the end of the last trading day".
 * Deliverable rule: standard iff shares_per_contract === standardShares (default 100) and no
 *   additional_underlyings. OCC Infomemo #26853 describes non-standard deliverables and root suffixes;
 *   some adjustments preserve standard deliverables. OI still counts contracts; different deliverables imply different exposure and are excluded here.
 * Strike key (convention, OSI 2010): strike * 1000 as an 8-digit integer in the ticker.
 * Intensity (visual convention, not a probability): linear v/max or ln(1+v)/ln(1+max), max = fixedMax
 *   or the largest known cell OI in the selected domain (expiration+side column or matrix).
 *   CME documents domain selection, not this normalization equation. scaleStatus 'provisional' = universe or OI incomplete, so the
 *   observed max may understate the true max; the UI must say so.
 * Cell rule: OI shown only when every standard reference contract at that location has one
 *   consistent valid OI, the backend attests a complete reference, and no snapshot ticker outside
 *   the reference falls on that location. Rejected known tickers taint their cells; unlocated
 *   selected-root rows block certainty, while unrelated foreign snapshots do not.
 * Coverage (own definition): standard contracts with valid OI / standard reference contracts.
 */
export const METHOD = 'oi-heatmap-v5';
export type Side = 'call' | 'put';
export interface Contract {
  ticker: string; symbol: string | null; expiration: string; strike: number; side: string;
  sharesPerContract: number | null;   // Null when the provider row omits it.
  extraDeliverables: number | null;          // additional_underlyings length; Massive omits it when empty.
}
export interface Observation extends Contract { oi: number | null }
export interface Input {
  symbol: string;
  observedMs: number;             // Capture time, NOT the OI effective date.
  oiDate: string | null;          // Null if provider/evidence does not identify it.
  populationId: string;
  referenceComplete: boolean;     // Backend attests full reference pagination in scope.
  expected: Contract[];           // Independent contract reference in requested scope.
  observations: Observation[];    // Snapshot rows; duplicates never add OI twice.
  scale?: 'linear' | 'log1p';
  normalization?: 'column' | 'matrix'; // Column = one expiration AND one side. Own visual convention.
  fixedMax?: number;              // Optional fixed ceiling for comparable snapshots.
  standardShares?: number;        // Standard equity/ETF deliverable; default 100.
  maxUnderlyingAgeMs?: number;    // Explicit caller policy, not an official threshold. Without it no price line.
  underlying?: Underlying | null; // Measured underlying price for the strike separator line.
}
/** Massive snapshot underlying_asset: price, last_updated (ns -> ms), timeframe. */
export interface Underlying { symbol: string; timestampPrecision: 'exact_ms' | 'approximate_ms'; price: number; updatedMs: number; timeframe: string | null; distinct: number }
export type CellState = 'known' | 'missing' | 'conflict' | 'partial' | 'excluded' | 'unknown_deliverable' | 'no_contract';
export interface Cell {
  id: string; strike: number; strikeMillis: number; expiration: string; side: Side;
  expectedContracts: number; receivedContracts: number; knownContracts: number;
  outsideReference: number;           // Snapshot tickers absent from reference at this location.
  unknownDeliverableContracts: number;
  nonStandardContracts: number;       // Excluded adjusted/unknown-deliverable tickers here.
  oi: number | null; observedOi: number | null; intensity: number | null;
  state: CellState; clipped: boolean;
  colorMax: number;
}
export interface Rejected { ticker: string | null; source: 'reference' | 'snapshot'; reason: string }
export interface Heatmap {
  method: string; symbol: string; populationId: string; observedMs: number;
  oiDate: string | null; scale: 'linear' | 'log1p'; maxOi: number; fixedMax: boolean;
  standardShares: number; strikes: number[]; expirations: string[]; cells: Cell[];
  duplicates: number; conflicts: number; invalidOi: number; unexpected: number;
  rejected: Rejected[]; nonStandard: string[];
  universeComplete: boolean; coverage: number | null; acceptedReferenceCoverage: number | null;
  scaleStatus: 'fixed' | 'complete' | 'provisional';
  normalization: 'column' | 'matrix';
  unknownDeliverable: string[];
  underlyingStatus: 'absent' | 'usable' | 'invalid' | 'foreign' | 'future' | 'stale' | 'unknown_timeframe';
  expectedContracts: number; knownContracts: number;
  underlying: Underlying | null;
  priceRow: number | null;        // 1..strikes.length: line after strikes[priceRow - 1] (strikes >= price above).
}
function date(value: unknown): value is string {
  return typeof value === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(value) &&
    Number.isFinite(Date.parse(value + 'T00:00:00Z')) &&
    new Date(value + 'T00:00:00Z').toISOString().slice(0, 10) === value;
}
export function strikeMillis(value: number): number {
  const scaled = value * 1000, key = Math.round(scaled);
  if (typeof value !== 'number' || !Number.isFinite(value) || value <= 0 || !Number.isSafeInteger(key) ||
      key > 99_999_999 || Math.abs(scaled - key) > 0.0001) throw Error('Invalid options strike');
  return key;
}
/** Returns a rejection reason, or null when the row is a valid call/put of this underlying. */
function problem(c: Contract, symbol: string): string | null {
  if (!c || typeof c.ticker !== 'string' || !c.ticker) return 'missing ticker';
  if (c.symbol !== symbol) return 'foreign or missing underlying';
  if (!date(c.expiration)) return 'invalid expiration';
  if (c.side !== 'call' && c.side !== 'put') return 'unsupported contract_type';
  let km: number;
  try { km = strikeMillis(c.strike); } catch { return 'invalid strike'; }
  const match = /^O:(.+)(\d{6})([CP])(\d{8})$/.exec(c.ticker);
  if (!match || match[2] !== c.expiration.slice(2).replaceAll('-', '') ||
      match[3] !== (c.side === 'call' ? 'C' : 'P') || Number(match[4]) !== km)
    return 'ticker/details mismatch';
  return null;
}
function locationKey(c: Contract): string {
  return JSON.stringify([c.symbol, strikeMillis(c.strike), c.expiration, c.side]);
}
function unknownDeliverable(c: Contract): boolean {
  return !Number.isFinite(c.sharesPerContract) || (c.sharesPerContract as number) <= 0 ||
    !Number.isSafeInteger(c.extraDeliverables) || (c.extraDeliverables as number) < 0;
}
function standard(c: Contract, shares: number): boolean {
  return c.sharesPerContract === shares && c.extraDeliverables === 0;
}
function recoverLocation(c: Contract, symbol: string): string | null {
  if (c?.symbol !== symbol || !date(c.expiration) || !['call','put'].includes(c.side)) return null;
  try { return locationKey(c); } catch { return null; }
}
export function heatIntensity(value: number, max: number, scale: 'linear' | 'log1p'): number {
  if (!['linear', 'log1p'].includes(scale) || !Number.isFinite(value) || value < 0 || !Number.isFinite(max) || max < 0 || (max === 0 && value > 0))
    throw Error('Invalid color domain');
  if (max === 0) return 0;
  return Math.min(1, scale === 'linear' ? value / max : Math.log1p(value) / Math.log1p(max));
}
export function buildOiHeatmap(input: Input): Heatmap {
  if (typeof input.symbol !== 'string' || !input.symbol.trim() || typeof input.populationId !== 'string' || !input.populationId.trim() || typeof input.referenceComplete !== 'boolean' ||
      !Array.isArray(input.expected) || !Array.isArray(input.observations) || !Number.isSafeInteger(input.observedMs) ||
      input.observedMs <= 0 || (input.oiDate !== null && !date(input.oiDate)))
    throw Error('Invalid provenance');
  const scale = input.scale ?? 'linear', shares = input.standardShares ?? 100;
  const normalization = input.normalization ?? 'column';
  if (!['column','matrix'].includes(normalization) || input.expected.length>100_000 || input.observations.length>100_000)
    throw Error('Invalid normalization or population limit');
  if (!['linear', 'log1p'].includes(scale) || !Number.isSafeInteger(shares) || shares <= 0 ||
      (input.fixedMax !== undefined && (!Number.isSafeInteger(input.fixedMax) || input.fixedMax <= 0)))
    throw Error('Invalid scale');
  const rejected: Rejected[] = [], nonStandard: string[] = [], unknown: string[] = [];
  const reference = new Map<string, Contract>(), groups = new Map<string, string[]>();
  const unknownAt = new Map<string, number>();
  const excludedAt = new Map<string, number>(), strikes = new Set<number>(), expirations = new Set<string>();
  const badReference = new Set<string>();
  const taintedLocations = new Set<string>();
  const taintedTickers = new Set<string>();
  let unlocatedReference = false, unlocatedSnapshot = false;
  for (const c of input.expected) {
    const why = problem(c, input.symbol);
    if (why) {
      rejected.push({ ticker: c?.ticker ?? null, source: 'reference', reason: why });
      if (typeof c?.ticker==='string') taintedTickers.add(c.ticker);
      const at=recoverLocation(c,input.symbol);if(at)taintedLocations.add(at);else unlocatedReference=true;
      continue;
    }
    const old = reference.get(c.ticker);
    if (old) {
      if (locationKey(old) !== locationKey(c) || old.sharesPerContract !== c.sharesPerContract || old.extraDeliverables !== c.extraDeliverables)
        badReference.add(c.ticker);
      continue;
    }
    reference.set(c.ticker, c);
  }
  for (const t of badReference) {
    rejected.push({ ticker: t, source: 'reference', reason: 'conflicting reference identity' });
    taintedLocations.add(locationKey(reference.get(t)!));
  }
  for (const t of taintedTickers) {const r=reference.get(t);if(r)taintedLocations.add(locationKey(r));}
  for (const c of reference.values()) {
    const at = locationKey(c);
    strikes.add(strikeMillis(c.strike)); expirations.add(c.expiration);
    if (unknownDeliverable(c)) { unknown.push(c.ticker); unknownAt.set(at, (unknownAt.get(at) ?? 0) + 1); continue; }
    if (!standard(c, shares)) { nonStandard.push(c.ticker); excludedAt.set(at, (excludedAt.get(at) ?? 0) + 1); continue; }
    const g = groups.get(at) ?? []; g.push(c.ticker); groups.set(at, g);
  }
  const received = new Map<string, number | null>(), conflicts = new Set<string>();
  const outside = new Map<string, number>();
  let duplicates = 0, unexpected = 0, invalidOi = 0;
  for (const c of input.observations) {
    const why = problem(c, input.symbol);
    if (why) {
      rejected.push({ ticker: c?.ticker ?? null, source: 'snapshot', reason: why });
      if (reference.has(c?.ticker)) conflicts.add(c.ticker);
      else if (c?.symbol===input.symbol || c?.symbol==null) {
        const at=recoverLocation(c,input.symbol);if(at)taintedLocations.add(at);else unlocatedSnapshot=true;
      }
      continue;
    }
    const ref = reference.get(c.ticker);
    if (!ref) {
      if (badReference.has(c.ticker)) continue;
      unexpected++; const at = locationKey(c); outside.set(at, (outside.get(at) ?? 0) + 1); continue;
    }
    if (!standard(ref, shares)) continue;     // Excluded by the reference deliverable.
    const valid = c.oi != null && Number.isSafeInteger(c.oi) && c.oi >= 0;
    if (c.oi != null && !valid) invalidOi++;
    const oi = valid ? c.oi : null;
    if (locationKey(ref) !== locationKey(c) ||
        (c.sharesPerContract !== null && c.sharesPerContract !== ref.sharesPerContract) ||
        c.extraDeliverables === null || c.extraDeliverables !== ref.extraDeliverables) {
      conflicts.add(c.ticker); continue;      // Snapshot identity disagrees with reference.
    }
    if (received.has(c.ticker)) {
      duplicates++;
      if (received.get(c.ticker) !== oi) conflicts.add(c.ticker);
    } else received.set(c.ticker, oi);
  }
  const universeComplete = input.referenceComplete && !unlocatedSnapshot && unexpected === 0 && unknown.length === 0 &&
    !rejected.some(r => r.source === 'reference');
  let knownContracts = 0;
  const keys = [...strikes].sort((a, b) => b - a), dates = [...expirations].sort();
  // Explicit resource limit, not a market rule. Prevent unbounded Cartesian allocation.
  if(keys.length*dates.length*2>500_000)throw Error('Heatmap exceeds 500000 cells; narrow the expiration scope');
  const cells: Cell[] = [];
  for (const strike of keys) for (const expiration of dates) for (const side of ['call', 'put'] as Side[]) {
    const id = JSON.stringify([input.symbol, strike, expiration, side]);
    const names = groups.get(id) ?? [];
    const nReceived = names.filter(n => received.has(n)).length;
    const good = names.filter(n => received.get(n) != null && !conflicts.has(n) && !taintedLocations.has(id));
    knownContracts += good.length;
    let observedOi: number | null = null;
    if (good.length) {
      observedOi = good.reduce((s, n) => s + (received.get(n) as number), 0);
      if (!Number.isSafeInteger(observedOi)) throw Error('OI sum exceeds safe integer precision');
    }
    const nOutside = outside.get(id) ?? 0;
    const state: CellState = taintedLocations.has(id) || names.some(n => conflicts.has(n)) ? 'conflict' :
      (unknownAt.get(id) ?? 0) > 0 ? 'unknown_deliverable' :
      !input.referenceComplete || unlocatedReference || unlocatedSnapshot || nOutside > 0 ? 'partial' : !names.length ? ((excludedAt.get(id) ?? 0) > 0 ? 'excluded' : 'no_contract') :
      good.length !== names.length ? 'missing' : 'known';
    cells.push({ id, strike: strike / 1000, strikeMillis: strike, expiration, side,
      expectedContracts: names.length, receivedContracts: nReceived, knownContracts: good.length,
      outsideReference: nOutside, unknownDeliverableContracts: unknownAt.get(id) ?? 0, nonStandardContracts: excludedAt.get(id) ?? 0,
      oi: state === 'known' ? observedOi : null, observedOi, intensity: null, state, clipped: false, colorMax:0 });
  }
  const maxOi = input.fixedMax ?? cells.reduce((m, c) => Math.max(m, c.oi ?? 0), 0);
  const columnMax = new Map<string,number>();
  for(const c of cells)if(c.oi!==null){const key=JSON.stringify([c.expiration,c.side]);columnMax.set(key,Math.max(columnMax.get(key)??0,c.oi));}
  for (const c of cells) if (c.oi !== null) {
    c.colorMax=input.fixedMax??(normalization==='matrix'?maxOi:columnMax.get(JSON.stringify([c.expiration,c.side]))??0);
    c.intensity = heatIntensity(c.oi, c.colorMax, scale); c.clipped = c.oi > c.colorMax;
  }
  const expectedContracts = [...groups.values()].reduce((s, g) => s + g.length, 0);
  const scaleStatus: Heatmap['scaleStatus'] = input.fixedMax !== undefined ? 'fixed' :
    universeComplete && expectedContracts > 0 && knownContracts === expectedContracts ? 'complete' : 'provisional';
  const supplied = input.underlying ?? null;
  let u = supplied;
  let underlyingStatus: Heatmap['underlyingStatus'] = u === null ? 'absent' : 'usable';
  // An unusable price only removes the line; it never discards the OI map.
  if (u !== null && (!Number.isFinite(u.price) || u.price <= 0 || !Number.isSafeInteger(u.updatedMs) || u.updatedMs <= 0 ||
      !Number.isSafeInteger(input.maxUnderlyingAgeMs) || (input.maxUnderlyingAgeMs as number) < 0 ||
      !['exact_ms', 'approximate_ms'].includes(u.timestampPrecision) || !Number.isSafeInteger(u.distinct) || u.distinct < 1)) {
    underlyingStatus = 'invalid'; u = null;
  }
  if (u !== null) {
    if (u.symbol !== input.symbol) underlyingStatus = 'foreign';
    else if (u.updatedMs > input.observedMs) underlyingStatus = 'future';
    else if (input.observedMs - u.updatedMs > (input.maxUnderlyingAgeMs as number)) underlyingStatus = 'stale';
    else if (u.timeframe !== 'REAL-TIME' && u.timeframe !== 'DELAYED') underlyingStatus = 'unknown_timeframe';
    if (underlyingStatus !== 'usable') u = null;
  }
  // Line position: count of strikes >= price; shown only when the price lies inside the strike range.
  const above = u === null ? 0 : keys.filter(k => k / 1000 >= u.price).length;
  const priceRow = u !== null && keys.length > 0 && u.price >= keys[keys.length - 1] / 1000 && u.price <= keys[0] / 1000 ? above : null;
  return { method: METHOD, symbol: input.symbol, populationId: input.populationId,
    observedMs: input.observedMs, oiDate: input.oiDate, scale, maxOi, fixedMax: input.fixedMax !== undefined,
    standardShares: shares, strikes: keys.map(k => k / 1000), expirations: dates, cells,
    duplicates, conflicts: conflicts.size, invalidOi, unexpected, rejected, nonStandard,
    universeComplete, scaleStatus, normalization, unknownDeliverable: unknown, underlyingStatus,
    expectedContracts, knownContracts, underlying: u, priceRow,
    acceptedReferenceCoverage: expectedContracts > 0 ? knownContracts / expectedContracts : null,
    coverage: universeComplete && expectedContracts > 0 ? knownContracts / expectedContracts : null };
}
/** Adapters for already-fetched Massive JSON, not network clients. Nothing is filled in. */
const deliverable = (r: any) => ({
  sharesPerContract: Number.isFinite(r?.shares_per_contract) ? r.shares_per_contract : null,
  extraDeliverables: r?.additional_underlyings === undefined ? 0 :
    Array.isArray(r.additional_underlyings) ? r.additional_underlyings.length : null });
export function fromMassiveReference(rows: any[]): Contract[] {
  return rows.map(r => ({ ticker: r?.ticker, symbol: r?.underlying_ticker ?? null,
    expiration: r?.expiration_date, strike: r?.strike_price, side: r?.contract_type, ...deliverable(r) }));
}
export function fromMassiveSnapshots(rows: any[]): Observation[] {
  return rows.map(r => ({ ticker: r?.details?.ticker, symbol: r?.underlying_asset?.ticker ?? null,
    expiration: r?.details?.expiration_date, strike: r?.details?.strike_price,
    side: r?.details?.contract_type, ...deliverable(r?.details),
    oi: r?.open_interest ?? null }));
}

/** Filter by identity and explicit age policy. Conflicting latest quotes are not selected.
 * Numeric JSON nanoseconds are approximate: exact millisecond boundaries are not guaranteed.
 * String/bigint timestamps permit exact integer conversion. No source is invented.
 */
export function underlyingFromMassiveSnapshots(rows: any[], symbol: string, observedMs: number, maxAgeMs: number): Underlying | null {
  if (!symbol || !Number.isSafeInteger(observedMs) || observedMs <= 0 || !Number.isSafeInteger(maxAgeMs) || maxAgeMs < 0)
    throw Error('Invalid price scope');
  const seen = new Map<string, Underlying>();
  for (const r of rows) {
    const a = r?.underlying_asset, ns = a?.last_updated;
    if (a?.ticker !== symbol || !Number.isFinite(a?.price) || a.price <= 0 ||
        !['REAL-TIME', 'DELAYED'].includes(a.timeframe)) continue;
    let updatedMs: number, timestampPrecision: Underlying['timestampPrecision'];
    if (typeof ns === 'bigint' || (typeof ns === 'string' && /^\d+$/.test(ns))) {
      const n = BigInt(ns); if (n <= 0n) continue;
      updatedMs = Number(n / 1000000n); timestampPrecision = 'exact_ms';
    } else {
      if (typeof ns !== 'number' || !Number.isFinite(ns) || ns <= 0 || !Number.isInteger(ns)) continue;
      updatedMs = Math.floor(ns / 1e6); timestampPrecision = Number.isSafeInteger(ns) ? 'exact_ms' : 'approximate_ms';
    }
    if (!Number.isSafeInteger(updatedMs) || updatedMs <= 0 || updatedMs > observedMs || observedMs - updatedMs > maxAgeMs) continue;
    const u: Underlying = {symbol, price:a.price, updatedMs, timeframe:a.timeframe, timestampPrecision, distinct:0};
    seen.set(JSON.stringify([u.price,u.updatedMs,u.timeframe,u.timestampPrecision]),u);
  }
  if (!seen.size) return null;
  const latestMs = [...seen.values()].reduce((m,u)=>Math.max(m,u.updatedMs),0);
  const latest = [...seen.values()].filter(u=>u.updatedMs === latestMs);
  if (new Set(latest.map(u=>JSON.stringify([u.price,u.timeframe]))).size !== 1) return null;
  const selected = latest.find(u=>u.timestampPrecision === 'exact_ms') ?? latest[0];
  return {...selected, distinct:seen.size};
}
/** Display rounding only (exact value in detail/aria): <1000 exact, else 0.1k / 0.1M. */
export function compactOi(v: number): string {
  if (!Number.isSafeInteger(v) || v < 0) throw Error('Invalid OI');
  if (v < 1000) return String(v);
  const k = Math.round(v / 100) / 10;
  if (k < 1000) return `${k.toLocaleString('es', { maximumFractionDigits: 1 })}k`;
  return `${(Math.round(v / 100_000) / 10).toLocaleString('es', { maximumFractionDigits: 1 })}M`;
}
/** Spanish labels for cell states, for any UI. */
export const STATE_LABELS_ES: Record<CellState, string> = { known: 'conocido', missing: 'falta OI',
  conflict: 'valores en conflicto', partial: 'universo de la celda no verificado', excluded: 'sin contratos estándar elegibles',
  unknown_deliverable: 'entregable pendiente de verificar', no_contract: 'sin contrato' };
