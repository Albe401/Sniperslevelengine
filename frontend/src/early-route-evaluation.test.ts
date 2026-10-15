import {it,expect} from 'vitest';
import {EarlyRouteEvaluation} from './early-route-evaluation';
import {highestArrivalProbability} from './arrival-probability-selection';
import {quantRouteFrame,QuantRouteMonitor} from './quant-route-live';
import type {IntradayRouteRead} from './session-intraday-route';
import type {LiveState} from './live';
import type {MarketEvidence} from './market-evidence';
import type {RouteResult} from './route-engine';
const start=Date.parse('2026-10-09T12:00:00-04:00');
const route={state:'range',direction:0,target:null,stop:null,level:null} as RouteResult;
const fixture=(now:number)=>({s:{symbol:'QQQ',startedMs:start,publishedMs:now,stocks:{phase:'streaming'},metrics:{session:'2026-10-09',phase:'Sesión RTH',asofMs:now},chartHistory:{sessions:[{date:'2026-10-09',openMs:start-150*60000,closeMs:start+240*60000}],minutes:[]}} as unknown as LiveState,m:{symbol:'QQQ',at:now,price:{ready:true,spot:100},flow:{at:now,signal:null,coverage:0},cvd:{at:now,signal:null,coverage:0},score:{components:[]},levels:[99,101].map(price=>({price,name:'WALL',role:'barrier',observed:start,asof:now,expiration:'2026-10-09',provisional:true,strength:70}))} as unknown as MarketEvidence,a:{value:1,percent:1,lastRangeRatio:1,endMs:now,reason:'fixture'}});
const bar=(time:number,close=100)=>({timestampMs:time,open:100,high:Math.max(100,close)+.1,low:Math.min(100,close)-.1,close,volume:100});
it('refreshes target-specific structure and excludes stale pressure and ATR without rewriting historical observations',()=>{
 const monitor=new EarlyRouteEvaluation(),f=fixture(start);
 f.m.flow={...f.m.flow,signal:.9,coverage:100,at:start-4000};f.m.cvd={...f.m.cvd,signal:.8,coverage:90,at:start};f.a.endMs=start-300001;
 monitor.read(f.s,f.m,route,f.a,[],true,start);
 const old=monitor.arrivals.snapshot(),call=old.find(r=>r.side===1)!;
 expect(call.features.routeDirection).toBe(1);expect(call.features.targetBps).toBe(100);expect(call.features.flowSignal).toBeNull();expect(call.features.cvdSignal).toBe(.8);expect(call.features.atrFiveMinutes).toBeNull();expect(call.features.targetAtr).toBeNull();
 const now=start+1800000,next=fixture(now);next.m.levels[1].price=102;next.m.levels[1].strength=25;
 monitor.read(next.s,next.m,route,next.a,[],true,now);
 const latest=monitor.arrivals.snapshot().filter(r=>r.side===1).at(-1)!;
 expect(latest.target).toBe(102);expect(latest.features.targetBps).toBe(200);expect(latest.features.targetStrength).toBe(25);
 expect(monitor.arrivals.snapshot().find(r=>r.id===call.id)?.target).toBe(101);expect(call.features.targetStrength).toBe(70);
 const noLevels={...next.m,levels:[]};expect(monitor.read(next.s,noLevels,route,next.a,[],true,now+1).probabilities).toEqual([]);
});
it('benchmarks the integrated exploratory engine, six horizon readings and winner selection',()=>{
 const monitor=new EarlyRouteEvaluation(),f=fixture(start);
 const begin=performance.now();for(let i=0;i<500;i++){const r=monitor.read(f.s,f.m,route,f.a,[],true,start);highestArrivalProbability(r.probabilities,start);}
 const elapsed=performance.now()-begin;console.info(`Arrival integrated: 500 updates in ${elapsed.toFixed(1)} ms; ${(elapsed/500).toFixed(3)} ms/update`);
 expect(elapsed).toBeLessThan(4000);expect(monitor.arrivals.snapshot().length).toBe(4);
});
it('benchmarks both live adapters with 330 closed candles and available EMA200 context',()=>{
 const now=start+120*60000,f=fixture(now),open=f.s.chartHistory!.sessions[0].openMs;
 f.s.chartHistory!.minutes=Array.from({length:330},(_,i)=>bar(open+i*60000));
 f.s.ticks={QQQ:{symbol:'QQQ',price:100,tradeMs:now,bid:99.99,ask:100.01,quoteMs:now}};
 f.m.levels.forEach(l=>l.provisional=false);f.m.cvd={...f.m.cvd,signal:.5,coverage:90};
 const early=new EarlyRouteEvaluation(),quant=new QuantRouteMonitor(),intraday={state:'waiting',side:'PENDIENTE'} as IntradayRouteRead;
 const begin=performance.now();for(let i=0;i<100;i++){
  early.read(f.s,f.m,route,f.a,f.s.chartHistory!.minutes,true,now);
  quant.read(quantRouteFrame(f.s,f.m,route,intraday,f.a,true,now),f.s.chartHistory!.minutes);
 }
 const elapsed=performance.now()-begin;console.info(`Both adapters + 330 candles: 100 updates in ${elapsed.toFixed(1)} ms; ${(elapsed/100).toFixed(3)} ms/update`);
 expect(elapsed).toBeLessThan(4000);expect(early.arrivals.snapshot()[0].features.ema200Bps).toBe(0);expect(quant.engine.snapshot()).toHaveLength(1);
});
it('evaluates a single visible PUT destination independently when the upper reference or ATR is unavailable',()=>{const monitor=new EarlyRouteEvaluation(),f=fixture(start);f.m.levels=f.m.levels.filter(l=>l.price<100);f.a.value=0;monitor.read(f.s,f.m,route,f.a,[],true,start);const observations=monitor.arrivals.snapshot();expect(observations.length).toBeGreaterThan(0);expect(observations.every(r=>r.side===-1&&r.target===99&&r.features.atrFiveMinutes===null)).toBe(true);expect(monitor.engine.snapshot()).toEqual([]);});
it('collects exploratory provisional references with unknown pressure kept unknown, then evaluates the first closed arrival',()=>{const monitor=new EarlyRouteEvaluation(),f=fixture(start);monitor.read(f.s,f.m,route,f.a,[],true,start);const frozen=monitor.engine.snapshot()[0];expect(frozen.upper).toBe(101);expect(frozen.features.flowSignal).toBeNull();const next=fixture(start+60000),r=monitor.read(next.s,next.m,route,next.a,[bar(start,101.1)],true,start+60000);expect(r.upper).toBe(1);expect(r.lower).toBe(0);expect(monitor.engine.snapshot()).toHaveLength(1);});
it('counts neither separately and invalidates interrupted data without claiming an error or an arrival',()=>{const monitor=new EarlyRouteEvaluation(),f=fixture(start);monitor.read(f.s,f.m,route,f.a,[],true,start);const next=fixture(start+600000),r=monitor.read(next.s,next.m,route,next.a,Array.from({length:10},(_,i)=>bar(start+i*60000)),true,start+600000);expect(r.neither).toBe(1);expect(r.pending).toBe(1);const failed=monitor.read(next.s,next.m,route,next.a,[],false,start+600001);expect(failed.missing).toBe(1);expect(failed.upper).toBe(0);});
it('finishes an existing observation near the close without opening one beyond the session',()=>{const monitor=new EarlyRouteEvaluation(),f=fixture(start);f.s.chartHistory!.sessions[0].closeMs=start+15*60000;monitor.read(f.s,f.m,route,f.a,[],true,start);const now=start+600000,next=fixture(now);next.s.chartHistory!.sessions[0].closeMs=start+15*60000;const r=monitor.read(next.s,next.m,route,next.a,Array.from({length:10},(_,i)=>bar(start+i*60000)),true,now);expect(r.neither).toBe(1);expect(r.pending).toBe(0);expect(r.missing).toBe(0);});
