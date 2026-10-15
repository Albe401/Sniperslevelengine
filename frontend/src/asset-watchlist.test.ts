import {describe,it,expect} from 'vitest';
import selection from '../../fixtures/curated-universe.json';
import {bundledAssets,searchAssets,assetResults} from './asset-watchlist';
describe('complete asset watchlist',()=>{
 it('ships the same full catalog as the engine, excluding unsupported index feeds',()=>{
  expect(Object.keys(bundledAssets).sort()).toEqual([...selection.activeSymbols].sort());
  expect(Object.keys(bundledAssets).length).toBeGreaterThan(76);
  expect(Object.keys(bundledAssets).length).toBeLessThan(500);
  for(const symbol of [...selection.originalSymbols,...selection.personalSymbols]) expect(bundledAssets[symbol]).toBeDefined();
  expect(searchAssets(bundledAssets)).toHaveLength(Object.values(bundledAssets).filter(a=>a.kind!=='INDEX').length);
  expect(searchAssets(bundledAssets).some(([s])=>s==='SPX')).toBe(false);
 });
 it('finds assets outside the five favorites by ticker and company name',()=>{
  expect(searchAssets(bundledAssets,' msft ').map(([s])=>s)).toContain('MSFT');
  expect(searchAssets(bundledAssets,'microsoft').map(([s])=>s)).toContain('MSFT');
  expect(searchAssets(bundledAssets,'amd').map(([s])=>s)).toContain('AMD');
  expect(searchAssets(bundledAssets,'hood').map(([s])=>s)).toContain('HOOD');
  expect(searchAssets(bundledAssets,'berkshire').map(([s])=>s)).toContain('BRK.B');
  expect(searchAssets(bundledAssets,'costco').map(([s])=>s)).toContain('COST');
  expect(searchAssets(bundledAssets,'aaon')).toHaveLength(0);
  for(const s of ['QQQ','SPY','GOOGL','NVDA','AAPL']) expect(bundledAssets[s]).toBeDefined();
 });
 it('filters sectors and handles no matches without dropping the catalog',()=>{
  const sector=bundledAssets.MSFT.sector!;
  expect(searchAssets(bundledAssets,sector).every(([,a])=>a.sector===sector)).toBe(true);
  expect(assetResults(bundledAssets,'zzzzzz')).toContain('Sin coincidencias');
  expect(searchAssets(bundledAssets,'')).toHaveLength(Object.values(bundledAssets).filter(a=>a.kind!=='INDEX').length);
 });
});
