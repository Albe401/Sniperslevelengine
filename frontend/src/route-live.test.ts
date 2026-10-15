import {it,expect} from 'vitest';
import {readRouteLive,routeJournal} from './route-live';
import type {LiveState} from './live';
const start=Date.UTC(2026,9,5,13,30),date='2026-10-05';
const base=Array.from({length:8},(_,i)=>({timestampMs:start+i*60000,open:100.2,high:100.3,low:100.1,close:100.2,volume:100,vwap:100.2}));
const seq=[{timestampMs:start+480000,open:100,high:100.5,low:99.8,close:100.4,volume:200},{timestampMs:start+540000,open:100.2,high:100.3,low:99.99,close:100.15,volume:80},{timestampMs:start+600000,open:100.15,high:100.7,low:100.1,close:100.6,volume:150}];
function state(symbol:string,minutes:typeof base):LiveState {
 const now=minutes.at(-1)!.timestampMs+60000;
 return {symbol,publishedMs:now,stocks:{phase:'streaming'},options:{phase:'streaming',lastEventMs:now},ticks:{[symbol]:{symbol,price:minutes.at(-1)!.close,tradeMs:now}},metrics:{session:date},flow:{intraday_windows:[]},
 chartHistory:{sessions:[{date,openMs:start,closeMs:start+390*60000}],minutes},optionSnapshotStatus:{phase:'ready',observedMs:now,horizons:[{mode:'DAY',observedMs:now,expirations:[date],structure:{complete:true,callWall:{strike:100},putWall:{strike:98},selected:{M1:102},routes:[]}}]}} as unknown as LiveState;
}
it('warmup does not manufacture a historical signal from current walls',()=>{const s=state('WARM',[...base,...seq.map(b=>({...b,vwap:b.close}))]);const r=readRouteLive(s,'WARM',true,s.publishedMs);expect(r.result.state).toBe('waiting');});
it('observes a new live sequence and captures inputs without enabling execution',()=>{let minutes=base.slice(),s=state('LIVE',minutes);readRouteLive(s,'LIVE',true,s.publishedMs);for(const b of seq){minutes=[...minutes,{...b,vwap:b.close}];s=state('LIVE',minutes);readRouteLive(s,'LIVE',true,s.publishedMs);}const r=readRouteLive(s,'LIVE',true,s.publishedMs);expect(r.result.state).toBe('sustained');expect(r.conclusion).toContain('Continuación alcista');expect(r.conclusion).toContain('apoyo incompleto');expect(r.assessment.quality).toBe('partial');expect(r.note).toContain('condiciones de ejecución pendientes');const log=JSON.parse(routeJournal());expect(log.executionEnabled).toBe(false);expect(log.records.at(-1).quality.combinedSignalValidated).toBe(false);});
it('disconnect clears the live route',()=>{const s=state('OFF',base);readRouteLive(s,'OFF',true,s.publishedMs);expect(readRouteLive(s,'OFF',false,s.publishedMs).result.state).toBe('waiting');});
it('reconnecting the same asset starts a new observation and cannot reuse its former route',()=>{
 let minutes=base.slice(),s=state('RECONNECT',minutes);s.startedMs=start;readRouteLive(s,s.symbol,true,s.publishedMs);
 for(const b of seq){minutes=[...minutes,{...b,vwap:b.close}];s=state('RECONNECT',minutes);s.startedMs=start;readRouteLive(s,s.symbol,true,s.publishedMs);}
 expect(readRouteLive(s,s.symbol,true,s.publishedMs).result.state).toBe('sustained');
 s.startedMs=start+600000;const fresh=readRouteLive(s,s.symbol,true,s.publishedMs);expect(fresh.result.state).toBe('waiting');expect(fresh.signals).toEqual([]);
});
it('late stale publications cannot preserve a signal',()=>{const s=state('STALE',base);expect(readRouteLive(s,'STALE',true,s.publishedMs+4000).result.state).toBe('waiting');});
it('never mixes next-session walls with intraday 0DTE context',()=>{const s=state('NEXT',base);s.optionSnapshotStatus.horizons[0].expirations=['2026-10-06'];const r=readRouteLive(s,'NEXT',true,s.publishedMs);expect(r.result.state).toBe('waiting');expect(r.reason).toContain('otro vencimiento');});
it('reads a weak put wall behind a rising price and requires an additional closed continuation',()=>{
 let minutes=base.slice();const symbol='WEAK';
 const frame=()=>{const s=state(symbol,minutes);s.optionSnapshotStatus.horizons[0].structure.callWall!.score=80;s.optionSnapshotStatus.horizons[0].structure.putWall!.score=45;return s;};
 let s=frame();readRouteLive(s,symbol,true,s.publishedMs);
 for(const b of seq){minutes=[...minutes,{...b,vwap:b.close}];s=frame();readRouteLive(s,symbol,true,s.publishedMs);}
 let r=readRouteLive(s,symbol,true,s.publishedMs);expect(r.structure.backing?.strength).toBe(45);expect(r.result.state).toBe('test');expect(r.result.direction).toBe(1);
 for(let i=0;i<10;i++)r=readRouteLive(s,symbol,true,s.publishedMs);expect(r.result.state).toBe('test');
 minutes=[...minutes,{timestampMs:start+660000,open:100.6,high:100.9,low:100.5,close:100.8,volume:150,vwap:100.8}];s=frame();r=readRouteLive(s,symbol,true,s.publishedMs);expect(r.result.state).toBe('sustained');
 s.optionSnapshotStatus.horizons[0].structure.putWall!.score=20;r=readRouteLive(s,symbol,true,s.publishedMs);expect(r.structure.backing?.strength).toBe(20);expect(r.result.direction).toBe(1);
});

it('records recent CVD support without changing the price route or adding execution permission',()=>{
 function run(symbol:string,withCvd:boolean){let minutes=base.slice();let s=state(symbol,minutes);readRouteLive(s,symbol,true,s.publishedMs);let result=readRouteLive(s,symbol,true,s.publishedMs);for(const b of seq){minutes=[...minutes,{...b,vwap:b.close}];s=state(symbol,minutes);s.metrics.phase='Sesión RTH';if(withCvd){const t=s.publishedMs;s.cvd={symbol,date,at:t,available:true,rejected:0,premarket:{buy:0,sell:0,unknown:0,delta:0,coveragePct:null,sinceMs:null,lastMs:null,changeOne:0,changeFive:0,gaps:0},regular:{buy:100,sell:800,unknown:100,delta:-700,coveragePct:90,sinceMs:start,lastMs:t,changeOne:-100,changeFive:-700,gaps:0},recentFive:{buy:100,sell:800,unknown:100,delta:-700,startMs:t-300000,endMs:t,complete:true}};}result=readRouteLive(s,symbol,true,s.publishedMs);}return result;}
 const baseResult=run('CVD_BASE',false),supported=run('CVD_WITH',true);
 expect(supported.result.state).toBe(baseResult.result.state);expect(supported.result.direction).toBe(baseResult.result.direction);expect(supported.result.target).toBe(baseResult.result.target);expect(supported.cvd.status).toBe('opposed');
 const last=JSON.parse(routeJournal()).records.at(-1);expect(last.cvdSupport.status).toBe('opposed');expect(last.cvdInput.symbol).toBe('CVD_WITH');expect(last.quality.combinedSignalValidated).toBe(false);
});
