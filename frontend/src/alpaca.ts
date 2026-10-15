import { invoke, isTauri } from './snapshot-observation';

/** Native backend owns credentials and the fixed PAPER host. No browser simulation fallback. */
function native<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) return Promise.reject(new Error('Alpaca Paper requiere la aplicación de escritorio.'));
  return invoke<T>(command, args);
}
export interface PaperOrderIntent {
  symbol: string;
  qty: number;
  limit_price: string;
  side: 'buy' | 'sell';
  /** Persist before submission; reuse for reconciliation, never blindly retry POST. */
  client_order_id: string;
}
export interface AlpacaOrder {
  id: string; client_order_id: string; symbol: string; status: string;
  qty: string; filled_qty: string; filled_avg_price: string | null;
}
export interface AlpacaPaperSnapshot {
  reconciliation: { verified: number; errors: number; batch_limit: number; orders: AlpacaOutboxEntry[] } | { error: string };
  environment: 'paper'; automatic_execution: false;
  account: { status: string; cash: string; equity: string; options_buying_power: string; options_trading_level: number; trading_blocked: boolean; account_blocked: boolean };
  positions: Array<{ symbol: string; qty: string; qty_available: string; side: string; unrealized_pl: string }>;
  orders: AlpacaOrder[]; orders_limit: number;
  clock: { timestamp: string; is_open: boolean; next_open: string; next_close: string };
}
export interface OptionContract {
  id: string; symbol: string; underlying_symbol: string; expiration_date: string;
  strike_price: string; type: 'call' | 'put'; tradable: boolean; status: string;
}
export interface AlpacaOutboxEntry {
  client_order_id: string; intent: string; state: 'pending' | 'acknowledged' | 'needs_review' | 'not_sent'; response: string | null; created_at: string; updated_at: string;
  broker_status: string | null; broker_response: string | null; last_error: string | null; checked_at_julian: number | null;
}
export const alpacaPaper = {
  reconcile: () => native<Exclude<AlpacaPaperSnapshot['reconciliation'], { error: string }>>('reconcile_alpaca_paper_orders'),
  outbox: () => native<AlpacaOutboxEntry[]>('get_alpaca_paper_outbox'),
  quote: (symbol: string) => native<{feed: 'opra'; data: {quotes: Record<string, {t: string; bp: number; ap: number; bs: number; as: number}>}}>('get_alpaca_option_quote', { symbol }),
  snapshot: () => native<AlpacaPaperSnapshot>('get_alpaca_paper_snapshot'),
  contracts: (underlying: string, expiration: string, pageToken?: string) => native<{option_contracts: OptionContract[]; next_page_token: string | null}>('get_alpaca_option_contracts', { underlying, expiration, pageToken }),
  order: (clientOrderId: string) => native<AlpacaOrder>('get_alpaca_paper_order', { clientOrderId }),
  submit: (intent: PaperOrderIntent) => native<AlpacaOrder>('submit_alpaca_paper_order', { intent }),
  cancel: (clientOrderId: string) => native<null>('cancel_alpaca_paper_order', { clientOrderId }),
};
