import {it,expect} from 'vitest';
import {trainArrivalProbability,readArrivalProbability,type ArrivalSample,type ArrivalModel} from './arrival-probability-engine';
const start=Date.parse('2026-01-05T15:00:00Z');
const rows:ArrivalSample[]=Array.from({length:3000},(_,i)=>{const at=start+Math.floor(i/30)*86400000+(i%30)*600000;return {id:'synthetic-'+i,symbol:'QQQ',side:1,horizon:'10m',at,deadline:at+600000,origin:100,target:101,features:{pressure:i%2?1:-1,missing:null},ready:true,session:new Date(at).toISOString().slice(0,10),source:'synthetic',outcome:i%2?'hit':'miss',resolvedAt:at+600000};});
const trained=trainArrivalProbability(rows,'QQQ',1,'10m'),model=trained.model!;
const frame=()=>({...rows[1],at:model.validatedThrough+86400000,deadline:model.validatedThrough+86400000+600000});
it('learns on chronological blocks, calibrates separately and never publishes synthetic performance',()=>{expect(model.brier).toBeLessThan(model.baselineBrier);expect(model.logLoss).toBeLessThan(model.baselineLogLoss);expect(model.trainedThrough).toBeLessThan(model.calibratedThrough);expect(model.calibratedThrough).toBeLessThan(model.validatedThrough);expect(trained.publishable).toBe(false);expect(readArrivalProbability(frame(),model).probability).toBeNull();});
it('re-evaluates probability as pressure changes only when a compatible validated model is supplied',()=>{const good={...model,source:'live' as const},up=readArrivalProbability(frame(),good),down=readArrivalProbability({...frame(),features:{pressure:-1,missing:null}},good);expect(up.probability).toBeGreaterThan(.8);expect(down.probability).toBeLessThan(.2);expect(readArrivalProbability(frame()).probability).toBeNull();});
it('rejects wrong asset, side, horizon, expired/future validation, corrupt counts, extrapolation and overflow',()=>{const good={...model,source:'live' as const};for(const bad of [{...good,symbol:'SPY'},{...good,side:-1},{...good,horizon:'session'},{...good,validatedThrough:frame().at},{...good,counts:{...good.counts,test:NaN}},{...good,bands:[]},{...good,temperature:Number.MIN_VALUE}])expect(readArrivalProbability(frame(),bad as ArrivalModel).probability).toBeNull();expect(readArrivalProbability({...frame(),at:good.validatedThrough+31*86400000,deadline:good.validatedThrough+31*86400000+600000},good).probability).toBeNull();expect(readArrivalProbability({...frame(),features:{pressure:100,missing:null}},good).probability).toBeNull();});
it('does not create an edge with constant uninformative features or repeated overlapping observations',()=>{const noEdge=trainArrivalProbability(rows.map(s=>({...s,features:{pressure:0,missing:null}})),'QQQ',1,'10m');expect(noEdge.publishable).toBe(false);expect(noEdge.model!.brier).toBeCloseTo(noEdge.model!.baselineBrier,10);expect(trainArrivalProbability([...rows,...rows],'QQQ',1,'10m').model!.counts).toEqual(model.counts);expect(trainArrivalProbability([],'QQQ',1,'10m').model).toBeUndefined();});
it('does not infer PUT as one minus CALL, and requires the correct endpoint deadline',()=>{expect(readArrivalProbability({...frame(),side:-1,target:99},{...model,source:'live'}).probability).toBeNull();expect(readArrivalProbability({...frame(),deadline:frame().at+1},{...model,source:'live'}).probability).toBeNull();});
it('rejects ambiguous duplicate probability bands instead of accepting the first one',()=>{const good={...model,source:'live' as const,bands:[...model.bands,...model.bands]};expect(readArrivalProbability(frame(),good).probability).toBeNull();});
it('responds to deteriorating structural, EMA and flow context in a controlled learned fixture, without claiming live calibration',()=>{
 const contextual=rows.map(s=>({...s,features:{structure:s.features.pressure,ema:s.features.pressure,cvd:s.features.pressure,volume:s.features.pressure}}));
 const result=trainArrivalProbability(contextual,'QQQ',1,'10m'),learned=result.model!;
 expect(result.publishable).toBe(false);
 // Only the unit-test clone passes the source gate; this model is never exported or installed.
 const testModel={...learned,source:'live' as const};
 const good={...frame(),features:{structure:1,ema:1,cvd:1,volume:1}},bad={...good,features:{structure:-1,ema:-1,cvd:-1,volume:-1}};
 const supported=readArrivalProbability(good,testModel),opposed=readArrivalProbability(bad,testModel);
 expect(supported.probability).toBeGreaterThan(.8);expect(opposed.probability).toBeLessThan(.2);
 expect(readArrivalProbability({...bad,ready:false},testModel).probability).toBeNull();
});
it('benchmarks guarded binary inference independently of offline training',()=>{
 const good={...model,source:'live' as const},f=frame();let total=0;
 const begin=performance.now();for(let i=0;i<10000;i++)total+=readArrivalProbability(f,good).probability??0;
 const elapsed=performance.now()-begin;console.info(`Arrival inference: 10000 reads in ${elapsed.toFixed(1)} ms; ${(elapsed/10000).toFixed(4)} ms/read`);
 expect(total).toBeGreaterThan(8000);expect(elapsed).toBeLessThan(3000);
});
