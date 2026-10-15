export interface Candle {
  timestampMs: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}
export interface Level { label: string; price: number; kind: string }

/** Canonical REST/AM minutes replace provisional A buckets; volumes never overlap. */
export function reconcileMinutes(minutes: Candle[], seconds: Candle[]): Candle[] {
  const merged = new Map(aggregate(seconds, 1).map(bar=>[bar.timestampMs,bar]));
  for (const bar of minutes) merged.set(bar.timestampMs,bar);
  return [...merged.values()].sort((a,b)=>a.timestampMs-b.timestampMs);
}
export interface Asset {
  symbol: string;
  name: string;
  previousClose: number;
  candles: Candle[];
  levels: Level[];
}
export interface Snapshot {
  schemaVersion: number;
  mode: 'demo';
  sessionLabel: string;
  assets: Asset[];
}

/** Same UTC buckets for all views; never aggregate an unfinished future candle. */
export function aggregate(candles: Candle[], minutes: number): Candle[] {
  if (!Number.isInteger(minutes) || minutes <= 0) throw new Error('Invalid interval');
  const buckets = new Map<number, Candle>();
  for (const c of [...candles].sort((a, b) => a.timestampMs - b.timestampMs)) {
    const key = Math.floor(c.timestampMs / (minutes * 60_000)) * minutes * 60_000;
    const old = buckets.get(key);
    if (!old) buckets.set(key, { ...c, timestampMs: key });
    else {
      old.high = Math.max(old.high, c.high);
      old.low = Math.min(old.low, c.low);
      old.close = c.close;
      old.volume += c.volume;
    }
  }
  return [...buckets.values()];
}

/** Illustrative bar-based VWAP, not the exchange tick VWAP. */
export function approximateVwap(candles: Candle[]): number | null {
  const volume = candles.reduce((n, c) => n + c.volume, 0);
  return volume > 0
    ? candles.reduce((n, c) => n + ((c.high + c.low + c.close) / 3) * c.volume, 0) / volume
    : null;
}

export function isSnapshot(value: unknown): value is Snapshot {
  if (!value || typeof value !== 'object') return false;
  const s = value as Snapshot;
  return s.schemaVersion === 1 && s.mode === 'demo' && typeof s.sessionLabel === 'string'
    && Array.isArray(s.assets) && s.assets.length > 0 && s.assets.every(a =>
      typeof a.symbol === 'string' && /^[A-Z]{1,6}$/.test(a.symbol)
      && typeof a.name === 'string' && Number.isFinite(a.previousClose) && a.previousClose > 0
      && Array.isArray(a.levels) && a.levels.every(l => typeof l.label === 'string' && Number.isFinite(l.price) && ['magnet', 'support', 'resistance'].includes(l.kind))
      && Array.isArray(a.candles) && a.candles.length > 0 && a.candles.every(c =>
        [c.timestampMs, c.open, c.high, c.low, c.close, c.volume].every(Number.isFinite)
        && c.volume >= 0 && c.low > 0 && c.high >= Math.max(c.open, c.close)
        && c.low <= Math.min(c.open, c.close)));
}

export const price = (value: number) => value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
export const compact = (value: number) => Intl.NumberFormat('en-US', { notation: 'compact', maximumFractionDigits: 1 }).format(value);
export const marketTime = (timestamp: number) => new Date(timestamp).toLocaleTimeString('en-US', { timeZone: 'America/New_York', hour: '2-digit', minute: '2-digit', hour12: false });
export const escapeHtml = (value: string) => value.replace(/[&<>"']/g, c => ({'&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'}[c]!));
