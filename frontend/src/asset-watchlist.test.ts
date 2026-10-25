import {describe,it,expect} from 'vitest';
import registry from '../../fixtures/legacy-universe.json';
import {bundledAssets,searchAssets,assetResults} from './asset-watchlist';
describe('configured snapshot asset watchlist',()=>{
 it('ships exactly the configured web catalog',()=>{
  expect(bundledAssets).toEqual(registry.assets);
  expect(Object.keys(bundledAssets).sort()).toEqual(['AAPL','AMZN','GOOGL','IWM','META','MSFT','NVDA','QQQ','SPY','TSLA']);
  expect(searchAssets(bundledAssets)).toHaveLength(10);
  expect(searchAssets(bundledAssets).some(([s])=>s==='SPX')).toBe(false);
 });
 it('searches the configured ticker and supplied company name without adding unsupported assets',()=>{
  expect(searchAssets(bundledAssets,' msft ').map(([s])=>s)).toEqual(['MSFT']);
  expect(searchAssets({...bundledAssets,MSFT:{...bundledAssets.MSFT,name:'Microsoft'}},'microsoft').map(([s])=>s)).toEqual(['MSFT']);
  expect(searchAssets(bundledAssets,'nvda').map(([s])=>s)).toEqual(['NVDA']);
  expect(searchAssets(bundledAssets,'aaon')).toHaveLength(0);
  for(const s of ['QQQ','SPY','GOOGL','NVDA','AAPL'])expect(bundledAssets[s]).toBeDefined();
 });
 it('filters supplied sectors and handles no matches without dropping the catalog',()=>{
  const assets={...bundledAssets,MSFT:{...bundledAssets.MSFT,sector:'Technology'}};
  expect(searchAssets(assets,'Technology').map(([s])=>s)).toEqual(['MSFT']);
  expect(assetResults(bundledAssets,'zzzzzz')).toContain('Sin coincidencias');
  expect(searchAssets(bundledAssets,'')).toHaveLength(10);
 });
});
