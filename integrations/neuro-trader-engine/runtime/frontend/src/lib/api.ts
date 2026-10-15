export const API_BASE = window.location.origin;
export const FORCE_DEMO = false;

/** "/ES" (how futures are shown) → "F:ES" (how the API receives them). */
export const apiSymbol = (s: string) => (s.startsWith("/") ? "F:" + s.slice(1) : s).toUpperCase();

export function wsUrl(symbol: string, maxDte: number, interval = 5): string {
  return `${API_BASE.replace(/^http/, "ws")}/gex/ws/${encodeURIComponent(apiSymbol(symbol))}?interval=${interval}&max_dte=${maxDte}`;
}
export const diagnosticsUrl = (symbol: string) => `${API_BASE}/diagnostics/${encodeURIComponent(apiSymbol(symbol))}`;
