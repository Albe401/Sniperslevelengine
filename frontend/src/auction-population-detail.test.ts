import {describe,it,expect} from 'vitest';
import {auctionPopulationDetail,type AuctionPopulationComparison} from './auction-population-detail';
describe('AMT population quantities',()=>{
 const fixture=():AuctionPopulationComparison=>({version:'amt-continuous-partition-20261013-v1',consolidated:{pocDiagnostics:{shares:652917.710397,largestTradeShares:650000,largestTradeConcentrationPct:99.553127}},continuous:{symbol:'QQQ',asOfMs:1000,pocLow:743,pocHigh:743.01,val:741,vah:744,totalVolume:100},separated:{shares:650000}});
 it('shows concentration and the alternate methodology without changing a POC',()=>{const r=auctionPopulationDetail('QQQ',1000,fixture());expect(r).toContain('99.55%');expect(r).toContain('POC 743.00–743.01');expect(r).toContain('no aceptación ni probabilidad');});
 it('does not present foreign or time-mismatched alternate profiles',()=>{const p=fixture();p.continuous!.symbol='SPY';expect(auctionPopulationDetail('QQQ',1000,p)).toContain('continuo pendiente');p.continuous!.symbol='QQQ';expect(auctionPopulationDetail('QQQ',1001,p)).toContain('continuo pendiente');});
 it('does not infer a profile or concentration from missing or invalid data',()=>{expect(auctionPopulationDetail('QQQ',1000)).toBe('');const p=fixture();p.continuous=null;p.consolidated!.pocDiagnostics!.shares=NaN;const r=auctionPopulationDetail('QQQ',1000,p);expect(r).not.toContain('Mayor print:');expect(r).toContain('continuo pendiente');});
});
