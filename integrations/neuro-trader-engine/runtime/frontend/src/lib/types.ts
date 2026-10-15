// Mirrors the payload built by backend/api/main.py (LiveHub._build).

export interface PackedGrid { unit: number; q: number[][]; }           // value ≈ q[i][j] · unit
export type Mode = "gamma" | "delta" | "charm" | "vanna";
export type Part = "c_all" | "c_ex0" | "p_all" | "p_ex0";                 // calls / puts, with / without 0DTE
export type Parts<T> = Record<Part, T>;

/** Dealer convention: sign applied to call and put open interest. */
export type Convention = "std" | "short" | "inv";
export const CONV_SIGNS: Record<Convention, { c: number; p: number }> = {
  std: { c: 1, p: -1 },     // customers buy puts & sell calls → dealers long calls, short puts (standard GEX)
  short: { c: -1, p: -1 },  // customers buy calls & puts → dealers short both
  inv: { c: -1, p: 1 },     // customers buy calls & sell puts
};

/** Per-strike gamma $ of each side as if dealers were LONG the open interest (all ≥ 0). */
export interface ProfileStrike {
  strike: number; c: number; p: number; c0: number; p0: number;
  // open interest and today's volume (contracts), calls/puts and their 0DTE part
  oi_c?: number; oi_p?: number; oi_c0?: number; oi_p0?: number;
  vol_c?: number; vol_p?: number; vol_c0?: number; vol_p0?: number;
  // Σ OI·Δ as if dealers were long (delta in contracts; ×100 = units of the underlying)
  dl_c?: number; dl_p?: number; dl_c0?: number; dl_p0?: number;
}

export interface Profile {
  prices: number[];                   // ascending, rows of every grid
  minutes: number[];                  // minutes since 09:30 ET, columns
  grids: Partial<Record<Mode, Parts<PackedGrid>>>;   // dealer-long exposures per side (vanna: v6.11+)
  fine: { prices: number[]; gamma: Parts<number[]> };   // gamma profile at the current minute
  strikes: ProfileStrike[];
  totals: { gex: Parts<number>; charm_flow: Parts<number>; gross_charm: { all: number; ex0: number };
    vanna_flow?: Parts<number> };            // $ dealers trade if implied vol FALLS 1 point (dealer-long)
  atm_iv: number;
  ref_price: number;
  q: number;
  range_pct: number;
  contracts_used: number;
  positions_priced?: number;
  has_0dte: boolean;
  /** Max pain of the nearest expiration (price where holders collect the least intrinsic value). */
  max_pain?: { strike: number; bdays: number; is_0dte: boolean; oi: number } | null;
  /** Dealer-long gamma $ and OI of each expiration (calls/puts apart; convention applied in the UI). */
  expiries?: ExpiryRow[];
  /** 1σ expected move: to today's close, and to the expiration nearest 5 sessions ahead. */
  em?: { day: { move: number; iv: number; minutes: number }; week: { move: number; iv: number; bdays: number } | null };
}

export interface ExpiryRow {
  expiration: string; bdays: number; kind: "0dte" | "daily" | "weekly" | "monthly" | "quarterly"; am: boolean;
  c: number; p: number; oi_c: number; oi_p: number;
}

export type AssetClass = "index" | "etf" | "stock" | "future";
export interface AssetInfo {
  class: AssetClass;
  symbol: string;
  exercise: "european" | "american";
  dividend_yield: number;
  dividend_source: string;
  strike_range_pct?: number;
  // futures only
  root?: string; name_es?: string; name_en?: string; proxy?: string; method?: "additive" | "ratio";
  tier?: "high" | "medium" | "low"; contract?: string; settlement_date?: string | null;
  price_source?: string; proxy_spot?: number; basis?: number; ratio?: number;
}

export interface KeyLevels {
  underlying_price: number; call_wall: number; put_wall: number; gamma_flip: number;
  zero_gamma: number | null; vol_trigger: number | null; high_vol_level: number;
  total_call_gex: number; total_put_gex: number; net_gex: number; total_dex: number;
  gamma_regime: string; regime_confidence: number; computation_ts: string;
  contracts_processed: number; expirations_used: string[];
}

export interface StrikeRow { strike: number; call_gex: number; put_gex: number; net_gex: number; call_oi: number; put_oi: number; }

export type SessionState = "open" | "pre_market" | "closed" | "closed_next_session";

export interface Market {
  spot: number;
  spot_source: string;
  prev_close: number | null;
  vix: number | null;
  timeframe: string;                  // REAL-TIME | DELAYED | MOCK | DEMO | UNKNOWN
  session_date: string;
  session_minutes: number;
  now_minute: number;
  session_state: SessionState;
  oi_date: string;
  generated_at: string;
  data_provider?: string;
  base_url?: string;
}

export interface LivePayload {
  symbol: string;
  asset: AssetInfo;
  key_levels: KeyLevels;
  by_strike: StrikeRow[];
  computation_ms: number;
  market: Market;
  path: [number, number][];           // [minute, price]
  path_source: string;
  profile: Profile | null;
  data_quality?: { contracts: number; with_iv: number; with_gamma: number; with_oi: number; fetch_ms: number; total_ms: number };
}

export interface ServerError { code: string; message: string; hint?: string; }

export type ConnStatus = "connecting" | "live" | "demo" | "error";
