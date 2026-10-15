import {expect,it} from 'vitest';
import {RouteProbabilityCollector,probabilityFeatures,validProbabilitySample} from './route-probability';
import type {LiveState} from './live';
import type {MarketEvidence} from './market-evidence';
import type {RouteResult} from './route-engine';
const start=Date.parse('2026-10-05T14:30:00Z'),day='2026-10-05';
function frame(now=start,direction=1){
 const s={symbol:'QQQ',startedMs:start-3600000,publishedMs:now,stocks:{phase:'streaming'},ticks:{QQQ:{symbol:'QQQ',price:101,tradeMs:now}},metrics:{session:day,phase:'Sesión RTH',asofMs:now,rvol:1.5},chartHistory:{sessions:[{date:day,openMs:start-3600000,closeMs:start+19800000}],minutes:[]}} as unknown as LiveState;
 const levels=[{name:'ANCHOR',price:100,expiration:day,strength:80,role:'put-wall'},{name:'TARGET',price:direction>0?105:97,expiration:day,role:'magnet'}];
 const e={symbol:'QQQ',at:now,price:{ready:true,spot:101},levels,flow:{at:now,signal:direction*.5,coverage:90},cvd:{at:now,signal:direction*.4,coverage:90},score:{call:direction>0?70:30,components:[{name:'Precio y VWAP',signal:direction*.7},{name:'Walls, imán y barreras',signal:direction*.2}]}} as unknown as MarketEvidence;
 const r={state:'sustained',direction,at:now,level:100,target:direction>0?105:97,stop:direction>0?100:102,key:'route',reason:''} as RouteResult;
 return {s,e,r};
}
const bar=(t:number,close=101)=>({timestampMs:t,open:101,high:Math.max(102,close),low:Math.min(100.5,close),close,volume:100});
it.each([1,-1])('records immutable joint inputs and resolves future closed target before stop (%s)',d=>{
 const c=new RouteProbabilityCollector(),f=frame(start,d);c.observe(f.s,f.e,f.r,[],true,start);
 const row=c.snapshot()[0];expect(validProbabilitySample(row)).toBe(true);expect(row.features.flowSignal).toBe(d*.5);expect(row.features.priceSignal).toBe(d*.7);expect(row.features.rvol).toBe(1.5);expect(row.features.iv).toBeNull();
 for(let i=0;i<50;i++)c.observe(f.s,f.e,f.r,[],true,start);expect(c.snapshot()).toHaveLength(1);
 const future=frame(start+60000,d);future.e.flow.signal=-d;c.observe(future.s,future.e,future.r,[bar(start,d>0?105:97)],true,start+60000);
 expect(c.snapshot()[0]).toMatchObject({outcome:'hit',target:d>0?105:97,features:row.features});expect(row.outcome).toBe('pending');
});
it('counts invalidation and timeout as failures, not reversed-side successes',()=>{
 const c=new RouteProbabilityCollector(),f=frame();c.observe(f.s,f.e,f.r,[],true,start);const next=frame(start+60000);c.observe(next.s,next.e,next.r,[bar(start,99)],true,start+60000);expect(c.snapshot()[0].outcome).toBe('invalidated');
 const t=new RouteProbabilityCollector();t.observe(f.s,f.e,f.r,[],true,start);
 for(let i=1;i<=10;i++){const x=frame(start+i*60000);t.observe(x.s,x.e,x.r,Array.from({length:i},(_,j)=>bar(start+j*60000)),true,start+i*60000);}
 expect(t.snapshot()[0].outcome).toBe('timeout');
});
it.each(['gap','disconnect','connection','symbol','clock','route'])('censors incomplete results rather than learning a false success or failure: %s',kind=>{
 const c=new RouteProbabilityCollector(),f=frame();c.observe(f.s,f.e,f.r,[],true,start);const x=frame(start+120000);let bars=[bar(start),bar(start+60000)];if(kind==='gap')bars=[bar(start+60000,106)];if(kind==='connection')x.s.startedMs++;if(kind==='symbol')x.s.symbol='SPY';
 c.observe(x.s,x.e,kind==='route'?{...x.r,state:'invalidated'}:x.r,bars,kind!=='disconnect',kind==='clock'?start-1:start+120000);expect(c.snapshot()[0].outcome).toBe('missing');
});
it('does not learn from an earlier historical target touch or an unfinished future bar',()=>{
 const c=new RouteProbabilityCollector(),f=frame();c.observe(f.s,f.e,f.r,[bar(start-60000,106)],true,start);expect(c.snapshot()[0].outcome).toBe('pending');
 const x=frame(start+30000);c.observe(x.s,x.e,x.r,[bar(start,106)],true,start+30000);expect(c.snapshot()[0].outcome).toBe('pending');
});
it('no incomplete pressure, near-close sample or overlapping forecast is admitted',()=>{
 for(const kind of ['pressure','clock','close'] as const){const c=new RouteProbabilityCollector(),f=frame();if(kind==='pressure')f.e.cvd.coverage=69;if(kind==='clock')f.e.flow.at=start+1;if(kind==='close')f.s.chartHistory!.sessions[0].closeMs=start+600000;c.observe(f.s,f.e,f.r,[],true,start);expect(c.snapshot()).toEqual([]);}
 const c=new RouteProbabilityCollector(),f=frame();c.observe(f.s,f.e,f.r,[],true,start);const x=frame(start+60000);c.observe(x.s,x.e,{...x.r,key:'different'},[bar(start,105)],true,start+60000);expect(c.snapshot()).toHaveLength(1);
});
it('one source contributes one feature; descriptive aggregate confluence is not an extra predictor',()=>{
 const f=frame(),features=probabilityFeatures(f.s,f.e,f.r,start);expect(features).not.toHaveProperty('confluence');expect(features).toHaveProperty('oiShare',null);expect(features).toHaveProperty('gammaShare',null);expect(features).toHaveProperty('blockPressure',null);expect(Object.values(features).every(v=>v===null||Number.isFinite(v))).toBe(true);
});
it('a delayed timeout is dated at its deadline so failures are not silently excluded from calibration',()=>{
 const c=new RouteProbabilityCollector(),f=frame();c.observe(f.s,f.e,f.r,[],true,start);const x=frame(start+600001);c.observe(x.s,x.e,x.r,Array.from({length:10},(_,j)=>bar(start+j*60000)),true,start+600001);const row=c.snapshot()[0];expect(row.outcome).toBe('timeout');expect(row.resolvedAt).toBe(row.deadline);
});
it('the route time limit is a failed scenario, not a censored loss that would inflate success',()=>{
 const c=new RouteProbabilityCollector(),f=frame();c.observe(f.s,f.e,f.r,[],true,start);const x=frame(start+480000);c.observe(x.s,x.e,{...x.r,state:'invalidated',reason:'La secuencia excedió diez minutos'},Array.from({length:8},(_,j)=>bar(start+j*60000)),true,start+480000);expect(c.snapshot()[0].outcome).toBe('invalidated');
});
