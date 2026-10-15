import { beforeEach, describe, expect, it, vi } from 'vitest';
const mocks = vi.hoisted(() => ({ invoke: vi.fn(), isTauri: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => mocks);
import { alpacaPaper } from './alpaca';
describe('Alpaca desktop bridge', () => {
  beforeEach(() => { vi.clearAllMocks(); mocks.isTauri.mockReturnValue(true); });
  it('refuses a misleading web preview connection', async () => {
    mocks.isTauri.mockReturnValue(false);
    await expect(alpacaPaper.snapshot()).rejects.toThrow('escritorio');
    expect(mocks.invoke).not.toHaveBeenCalled();
  });
  it('preserves contract pagination and explicit expiry', async () => {
    await alpacaPaper.contracts('QQQ', '2026-09-25', 'next');
    expect(mocks.invoke).toHaveBeenCalledWith('get_alpaca_option_contracts', {underlying: 'QQQ', expiration: '2026-09-25', pageToken: 'next'});
  });
  it('does not silently retry uncertain order submission', async () => {
    mocks.invoke.mockRejectedValueOnce(new Error('timeout'));
    await expect(alpacaPaper.submit({symbol: 'QQQ260925C00600000', qty: 1, limit_price: '1.25', side: 'buy', client_order_id: 'snipers-test'})).rejects.toThrow('timeout');
    expect(mocks.invoke).toHaveBeenCalledTimes(1);
  });
});
