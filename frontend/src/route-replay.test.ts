import {it,expect} from 'vitest';
import {replayRoutePrice,historicalPriceState} from './route-replay';
import type {ChartHistory} from './candle-series';
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
const real=JSON.parse(readFileSync(process.env.SNIPERS_AUDIT_FIXTURE_ROOT?resolve(process.env.SNIPERS_AUDIT_FIXTURE_ROOT,'../../fixtures/aapl-2026-09-30-bars.json'):new URL('../../fixtures/aapl-2026-09-30-bars.json',import.meta.url),'utf8'));
import {SessionIntradayRoute,sessionRouteLevels} from './session-intraday-route';
import {readMarketEvidence} from './market-evidence';
const open=Date.parse('2026-09-30T13:30:00Z'),close=Date.parse('2026-09-30T20:00:00Z');
const session={date:'2026-09-30',openMs:open,closeMs:close};
const history=():ChartHistory=>({sessions:[session],minutes:Array.from({length:10},(_,i)=>({timestampMs:open+i*60000,open:100+i,high:101+i,low:99+i,close:100.5+i,volume:100,vwap:100.3+i}))});
it('does not expose a future candle or options snapshot to a historical price reading',()=>{
 const s=historicalPriceState(history(),'AAPL',open+5*60000)!;
 expect(s.chartHistory!.minutes).toHaveLength(5);expect(s.options.phase).toBe('disconnected');expect(s.flow.intraday_windows).toEqual([]);
 expect(s.optionSnapshotStatus).toBeUndefined();expect(s.levelDeltas).toBeUndefined();
});
it('future price spikes cannot change earlier readings',()=>{
 const a=history(),b=history();b.minutes[9]={...b.minutes[9],open:999,high:1001,low:998,close:1000};
 const left=replayRoutePrice(a,'AAPL',session.date,close),right=replayRoutePrice(b,'AAPL',session.date,close);
 expect(left.causal).toBe(true);expect(right.points.slice(0,9)).toEqual(left.points.slice(0,9));
});
it('does not count unfinished bars',()=>{const r=replayRoutePrice(history(),'AAPL',session.date,open+5*60000+30000);expect(r.points).toHaveLength(5);});
it('a missing minute interrupts the reading instead of fabricating persistence',()=>{const h=history();h.minutes.splice(5,1);const r=replayRoutePrice(h,'AAPL',session.date,close);expect(r.gaps).toBe(1);expect(r.points[5].reading.ready).toBe(false);});
it('keeps missing VWAP missing',()=>{const h=history();delete h.minutes[1].vwap;expect(historicalPriceState(h,'AAPL',open+5*60000)!.metrics.vwap).toBeNull();});
it('does not borrow candles from a different session',()=>{expect(replayRoutePrice(history(),'AAPL','2026-10-01',close).points).toHaveLength(0);});
it('replays saved real Massive AAPL RTH candles with no future information',()=>{
 const h:ChartHistory={sessions:[session],minutes:real.results.map(v=>({timestampMs:v.t,open:v.o,high:v.h,low:v.l,close:v.c,volume:v.v,vwap:v.vw}))};
 const r=replayRoutePrice(h,real.symbol,real.date,close);
 const available=real.results.filter(v=>v.t>=open&&v.t<close&&v.l>0&&v.l<=Math.min(v.o,v.c)&&v.h>=Math.max(v.o,v.c)); expect(r.points).toHaveLength(new Set(available.map(v=>v.t)).size); expect(r.points.length).toBeGreaterThan(0);expect(r.causal).toBe(true);expect(r.scope).toBe('price-only');
 expect(r.points.every(p=>p.at<=close)).toBe(true);
 console.log('REAL_ROUTE_PRICE_REPLAY',JSON.stringify({source:real.source,symbol:r.symbol,date:r.date,closes:r.points.length,ready:r.ready,pending:r.rejected,gaps:r.gaps,causal:r.causal,scope:r.scope}));
});
it('audits the session route and ATR against real closes without manufacturing missing CVD',()=>{
 const h:ChartHistory={sessions:[session],minutes:real.results.map(v=>({timestampMs:v.t,open:v.o,high:v.h,low:v.l,close:v.c,volume:v.v,vwap:v.vw}))};
 const engine=new SessionIntradayRoute();let count=0,atrReady=0;
 for(const at of [...new Set(h.minutes.filter(v=>v.timestampMs>=open&&v.timestampMs+60000<close).map(v=>v.timestampMs+60000))].sort((a,b)=>a-b)){
  const s=historicalPriceState(h,real.symbol,at);if(!s)continue;
  const market=readMarketEvidence(s,real.symbol,true,at),r=engine.read(s,real.symbol,true,market,at);
  expect(r.state).not.toBe('confirmed');expect(r.signals).toEqual([]);expect(r.executionEnabled).toBe(false);
  expect(sessionRouteLevels({...s,chartHistory:h},at)).toEqual(sessionRouteLevels(s,at));
  if(r.atr?.value!=null)atrReady++;
  if(r.target!=null)expect(sessionRouteLevels(s,at).some(v=>v.price===r.target)).toBe(true);
  count++;
 }
 expect(count).toBeGreaterThan(100);expect(atrReady).toBeGreaterThan(0);
 console.log('REAL_SESSION_ROUTE_AUDIT',JSON.stringify({symbol:real.symbol,date:real.date,closes:count,atrReady,confirmed:0,missingHistoricalCvd:true,causalLevels:true}));
});
