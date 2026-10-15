import {expect,it} from 'vitest';
import {readRouteLive,routeChartReading} from './route-live';
import {routeChartScenario,routeChartStatus} from './route-chart-scenario';
import type {LiveState} from './live';
const start=Date.UTC(2026,9,5,13,30),date='2026-10-05';
type Bar={timestampMs:number;open:number;high:number;low:number;close:number;volume:number;vwap:number};
function frame(symbol:string,bars:Bar[]):LiveState {
 const now=bars.at(-1)!.timestampMs+60000;
 return {symbol,publishedMs:now,stocks:{phase:'streaming'},options:{phase:'streaming',lastEventMs:now},ticks:{[symbol]:{symbol,price:bars.at(-1)!.close,tradeMs:now}},metrics:{session:date,phase:'Sesión RTH'},flow:{intraday_windows:[]},chartHistory:{sessions:[{date,openMs:start,closeMs:start+390*60000}],minutes:bars},optionSnapshotStatus:{phase:'ready',observedMs:now,horizons:[{mode:'DAY',observedMs:now,expirations:[date],structure:{complete:true,callWall:{strike:100},putWall:{strike:98},selected:{M1:102},routes:[]}}]}} as unknown as LiveState;
}
function run(symbol:string,direction=1){
 let bars:Bar[]=Array.from({length:8},(_,i)=>({timestampMs:start+i*60000,open:100.2,high:100.3,low:100.1,close:100.2,volume:100,vwap:100.2}));
 const sequence=[{open:100,high:100.5,low:99.8,close:100.4,volume:200},{open:100.2,high:100.3,low:99.99,close:100.15,volume:80},{open:100.15,high:100.7,low:100.1,close:100.6,volume:150}];
 const reflect=(b:Bar):Bar=>direction===1?b:{...b,open:200-b.open,high:200-b.low,low:200-b.high,close:200-b.close,vwap:200-b.vwap};
 bars=bars.map(reflect);
 const state=()=>{const s=frame(symbol,bars);if(direction===-1){const h=s.optionSnapshotStatus.horizons[0];h.structure.callWall!.strike=102;h.structure.putWall!.strike=100;h.structure.selected.M1=98;}return s;};
 let s=state();readRouteLive(s,symbol,true,s.publishedMs);
 const stages=[];
 for(const [i,b] of sequence.entries()){bars.push(reflect({...b,timestampMs:start+(8+i)*60000,vwap:b.close}));s=state();stages.push(readRouteLive(s,symbol,true,s.publishedMs).result.state);}
 return {s,bars,state,stages};
}
it.each([1,-1])('engine → panel cache → chart retains the same %s direction, target and invalidation',direction=>{
 const symbol='BRIDGE_'+direction,{s,stages}=run(symbol,direction);expect(stages).toEqual(['recovery','test','sustained']);
 const panel=readRouteLive(s,symbol,true,s.publishedMs),chart=routeChartReading(symbol,s.publishedMs)!;
 expect(chart.result).toEqual(panel.result);expect(routeChartScenario(chart.result,s.ticks[symbol].price??undefined,s.publishedMs,true)).toEqual({target:direction===1?102:98,stop:direction===1?99.97:100.03,direction});
 expect(routeChartStatus(chart.result,true)).toContain(direction===1?'CALL':'PUT');
});
it('fresh closed continuations keep the same route visible beyond 90 seconds after initial confirmation',()=>{
 const symbol='CONTINUE',{bars,state}=run(symbol);
 for(let i=11;i<=13;i++){bars.push({timestampMs:start+i*60000,open:100.6,high:101.1,low:100.5,close:100.8,volume:110,vwap:100.8});const s=state();const result=readRouteLive(s,symbol,true,s.publishedMs).result;expect(result.state).toBe('sustained');expect(result.at).toBe(s.publishedMs);expect(routeChartScenario(routeChartReading(symbol,s.publishedMs)?.result,100.8,s.publishedMs,true)).toBeDefined();}
});
it.each(['reached','invalidated'] as const)('a real closed %s event clears the chart projection and labels its actual state',event=>{
 const symbol='END_'+event,{bars,state}=run(symbol);const close=event==='reached'?102.1:99.8;
 bars.push({timestampMs:start+11*60000,open:100.6,high:Math.max(100.7,close+.1),low:Math.min(100.4,close-.1),close,volume:200,vwap:close});const s=state();const panel=readRouteLive(s,symbol,true,s.publishedMs);const r=routeChartReading(symbol,s.publishedMs)!.result;
 expect(r).toEqual(panel.result);expect(r.state).toBe(event);expect(routeChartScenario(r,close,s.publishedMs,true)).toBeUndefined();expect(routeChartStatus(r,true)).toContain(event==='reached'?'objetivo alcanzado':'invalidada');
});
it('chart reads are immutable, do not advance the engine and expire after three seconds',()=>{
 const symbol='READ_ONLY',{s}=run(symbol),now=s.publishedMs;const first=routeChartReading(symbol,now)!;first.result.target=999;
 for(let i=0;i<30;i++)expect(routeChartReading(symbol,now)?.result.target).toBe(102);
 expect(routeChartReading(symbol,now+3001)).toBeUndefined();expect(routeChartStatus(routeChartReading(symbol,now+3001)?.result,true)).toBe('Ruta intradía · pendiente');expect(routeChartReading(symbol,now-1)).toBeUndefined();expect(routeChartReading('OTHER',now)).toBeUndefined();
});
it('switching asset, returning, or disconnecting cannot retain the prior signal',()=>{
 const {s}=run('SWITCH');readRouteLive(frame('OTHER',s.chartHistory!.minutes as Bar[]),'OTHER',true,s.publishedMs);
 expect(routeChartReading('SWITCH',s.publishedMs)).toBeUndefined();expect(readRouteLive(s,'SWITCH',true,s.publishedMs).result.state).toBe('waiting');
 expect(readRouteLive(s,'SWITCH',false,s.publishedMs).result.state).toBe('waiting');expect(routeChartScenario(routeChartReading('SWITCH',s.publishedMs)?.result,100.6,s.publishedMs,true)).toBeUndefined();
});
it.each(['stale','next-expiry','corrected','gap'] as const)('the actual %s input cannot leave an active chart projection',kind=>{
 const symbol='GUARD_'+kind,{s,bars,state}=run(symbol);let now=s.publishedMs;
 if(kind==='stale')now+=4000;
 if(kind==='next-expiry')s.optionSnapshotStatus.horizons[0].expirations=['2026-10-06'];
 if(kind==='corrected')s.chartHistory!.minutes.at(-1)!.close=100.61;
 if(kind==='gap'){bars.push({timestampMs:start+12*60000,open:100.6,high:100.9,low:100.5,close:100.8,volume:100,vwap:100.8});Object.assign(s,state());now=s.publishedMs;}
 const p=readRouteLive(s,symbol,true,now);expect(p.result.state).toBe('waiting');expect(routeChartScenario(routeChartReading(symbol,now)?.result,100.6,now,true)).toBeUndefined();
});
