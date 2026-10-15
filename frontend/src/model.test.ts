import { describe, expect, it } from 'vitest';
import fixture from '../../fixtures/demo-market.json';
import { aggregate, approximateVwap, isSnapshot, marketTime, reconcileMinutes } from './model';

describe('market view contract', () => {
  it('reconciles official minutes without adding streamed volume twice',()=>{
    const bar={timestampMs:60_000,open:10,high:12,low:9,close:11,volume:20};
    const merged=reconcileMinutes([bar],[{...bar,volume:5},{...bar,timestampMs:61_000,volume:8},{...bar,timestampMs:120_000,volume:7}]);
    expect(merged).toHaveLength(2);
    expect(merged[0].volume).toBe(20);
    expect(merged[1].volume).toBe(7);
    expect(aggregate(merged,5)[0].volume).toBe(27);
  });
  it('accepts demo data and refuses unvalidated live payloads', () => {
    expect(isSnapshot(fixture)).toBe(true);
    expect(isSnapshot({ ...fixture, mode: 'live' })).toBe(false);
    expect(isSnapshot({ assets: [] })).toBe(false);
  });
  it('preserves OHLC and volume across 2-minute buckets', () => {
    const input = fixture.assets[0].candles.slice(0, 4);
    const bars = aggregate(input, 2);
    expect(bars).toHaveLength(2);
    expect(bars[0].open).toBe(input[0].open);
    expect(bars[0].close).toBe(input[1].close);
    expect(bars[0].high).toBe(Math.max(input[0].high, input[1].high));
    expect(bars[0].volume).toBe(input[0].volume + input[1].volume);
    expect(bars.reduce((n, c) => n + c.volume, 0)).toBe(input.reduce((n, c) => n + c.volume, 0));
  });
  it('handles unsorted bars, empty volume and invalid intervals', () => {
    const input = fixture.assets[0].candles.slice(0, 4);
    expect(aggregate([...input].reverse(), 2)).toEqual(aggregate(input, 2));
    expect(approximateVwap([])).toBeNull();
    expect(() => aggregate(input, 0)).toThrow();
    expect(aggregate([], 5)).toEqual([]);
  });
  it('formats winter and summer sessions in exchange time', () => {
    expect(marketTime(Date.UTC(2026, 0, 5, 14, 30))).toBe('09:30');
    expect(marketTime(Date.UTC(2026, 8, 23, 13, 30))).toBe('09:30');
  });
});
