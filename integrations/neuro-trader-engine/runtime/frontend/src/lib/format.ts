export function fmtMoney(v: number): string {
  const a = Math.abs(v), s = v < 0 ? "−" : "+";
  if (a >= 1e9) return s + "$" + (a / 1e9).toFixed(a >= 1e10 ? 1 : 2) + "B";
  if (a >= 1e6) return s + "$" + (a / 1e6).toFixed(a >= 1e8 ? 0 : 1) + "M";
  if (a >= 1e3) return s + "$" + (a / 1e3).toFixed(0) + "K";
  return s + "$" + a.toFixed(0);
}
export const unsigned = (s: string) => s.replace(/^[+−]/, "");

/** Price level: no decimals above 1000, otherwise 0–1 as needed (strikes like 182.5). */
export function fmtLvl(p: number | null | undefined): string {
  if (p === null || p === undefined || !isFinite(p)) return "—";
  if (Math.abs(p) >= 1000) return Math.round(p).toLocaleString("en-US");
  return Math.abs(p - Math.round(p)) < 1e-6 ? String(Math.round(p)) : p.toFixed(1);
}
export const fmtPx = (p: number) => p.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 });

export function hhmm(minute: number): string {
  const t = 570 + Math.round(minute), h = Math.floor(t / 60), m = t % 60;
  return (h < 10 ? "0" : "") + h + ":" + (m < 10 ? "0" : "") + m;
}
export function pct(a: number, b: number): string {
  const p = (a - b) / b * 100;
  return (p >= 0 ? "+" : "−") + Math.abs(p).toFixed(2) + "%";
}
/** 1234567 → "1.23M", 45210 → "45.2K" */
export function fmtCompact(v: number): string {
  const a = Math.abs(v);
  if (a >= 1e9) return (v / 1e9).toFixed(2) + "B";
  if (a >= 1e6) return (v / 1e6).toFixed(2) + "M";
  if (a >= 1e3) return (v / 1e3).toFixed(1) + "K";
  return String(Math.round(v));
}
export const clamp = (x: number, a: number, b: number) => (x < a ? a : x > b ? b : x);
