import {expect,it} from 'vitest';
import {probabilityVersion,probabilityEndpoint,type ProbabilitySample} from './route-probability';
import {trainRouteProbability,predictRouteProbability} from './route-probability-model';
import {probabilityReading} from './route-probability-reading';
import {directionBadge,combineDirection} from './direction-scanner';
const start=Date.parse('2026-01-01T14:30:00Z');
function sample(i:number):ProbabilitySample {
 const day=Math.floor(i/20),j=i%20,at=start+day*86400000+j*600000,direction=i%2?1:-1,signal=j<10?1:-1;
 return {id:'TEST-'+i,version:probabilityVersion,endpoint:probabilityEndpoint,source:'live',symbol:'QQQ',session:new Date(at).toISOString().slice(0,10),connection:1,at,deadline:at+600000,direction,price:100,target:100+direction,stop:100-direction,lastBar:at-60000,features:{priceSignal:signal,flowSignal:signal*.5,cvdSignal:signal*.3,routeDirection:direction,iv:null},outcome:signal>0?'hit':'timeout',resolvedAt:at+600000,reason:'Synthetic test fixture; never deployed'};
}
// Deterministic fixtures validate the algorithm, never a claimed market win rate.
const rows=Array.from({length:2000},(_,i)=>sample(i));
const result=trainRouteProbability(rows,'QQQ');
it('refuses empty data, legacy endpoints and insufficient days',()=>{
 expect(trainRouteProbability([],'QQQ').model).toBeUndefined();expect(trainRouteProbability(rows.slice(0,200),'QQQ').model).toBeUndefined();expect(trainRouteProbability(rows.map(r=>({...r,endpoint:'return-5m'})),'QQQ').model).toBeUndefined();
});
it('fits joint inputs, calibrates later days, and validates on a third untouched temporal block',()=>{
 const m=result.model!;expect(m).toBeDefined();expect(m.trainedThrough).toBeLessThan(m.calibratedThrough);expect(m.calibratedThrough).toBeLessThan(m.validatedThrough);expect(m.brier).toBeLessThan(m.baselineBrier);expect(m.bins.some(b=>b.usable&&b.n>=100)).toBe(true);expect(m.columns).toEqual(Object.keys(rows[0].features).sort());
});
it('duplicates, overlapping rows and missing outcomes cannot inflate the sample',()=>{
 const r=trainRouteProbability([...rows,...rows,...rows.map(r=>({...r,id:r.id+'-overlap',at:r.at+1,deadline:r.deadline+1})),...rows.map(r=>({...r,id:r.id+'-missing',outcome:'missing'}))],'QQQ');expect(r.counts.samples).toBe(2000);
});
it('predicts only future supported same-asset/version/range scenarios',()=>{
 const m=result.model!,r=sample(2000);const p=predictRouteProbability(r,m,r.at);expect(p.percent).not.toBeNull();expect(p.n).toBeGreaterThanOrEqual(100);
 for(const model of [undefined,{...m,symbol:'SPY'},{...m,endpoint:'return-5m'},{...m,validatedThrough:r.at},{...m,brier:1},{...m,bins:[]},{...m,weights:m.weights.map(()=>NaN)}])expect(predictRouteProbability(r,model,r.at).percent).toBeNull();
 expect(predictRouteProbability({...r,features:{...r.features,priceSignal:100}},m,r.at).percent).toBeNull();expect(predictRouteProbability({...r,features:{...r.features,iv:.2}},m,r.at).percent).toBeNull();
});
it('a 70 confluence score never turns into 70 probability without a validated model',()=>{
 const row={...sample(2000),outcome:'pending' as const,resolvedAt:null};const reading=probabilityReading(row,undefined,row.at);expect(reading.call).toBeNull();expect(reading.put).toBeNull();
 const html=directionBadge(combineDirection(.5,.5,.5,60),undefined,reading),visible=html.replace(/<details[\s\S]*?<\/details>/g,'');expect(visible).toContain('CONF. PROVISIONAL');expect(visible).toContain('Sin calibrar');expect(visible).not.toContain('PROB. 10 MIN');expect(html).toContain('Confluencia interna');
});
it('CALL and PUT probability are separate outcomes, not complementary shares',()=>{
 const m=result.model!,row={...sample(2001),outcome:'pending' as const,resolvedAt:null};const read=probabilityReading(row,m,row.at);expect(read.call).not.toBeNull();expect(read.put).toBeNull();expect(probabilityReading(row,m,row.deadline+1).call).toBeNull();
});

