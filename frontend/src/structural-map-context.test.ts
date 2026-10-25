import {describe,expect,it} from 'vitest';
import {writeFileSync} from 'node:fs';
import {join} from 'node:path';
import real from './structural-map-context.real.json';
import {buildStructuralMapContext,type StructureContextInput} from './structural-map-context';
import {readMassiveExposure} from './massive-exposure';
import type {LiveState} from './live';
import type {MarketEvidence} from './market-evidence';

function recorded(){return {s:structuredClone(real.state) as unknown as LiveState,input:structuredClone(real.structure) as unknown as StructureContextInput,now:real.now};}
function synthetic(){
 const f=recorded();f.now=Date.parse('2026-10-26T15:30:00-04:00');f.s.publishedMs=f.now;f.input.publishedMs=f.now;f.input.evaluatedMs=f.now;f.s.flow.asof_ms=f.now-1000;
 for(const c of f.input.cards){c.endMs=f.now-60000;c.reference=false;}
 const h=f.s.optionSnapshotStatus.horizons[0];h.mode='DAY';h.observedMs=f.now-1000;h.expirations=['2026-10-26'];f.s.optionSnapshotStatus.observedMs=h.observedMs;f.s.optionSnapshotStatus.horizons=[h];
 return f;
}
function evidence(f:ReturnType<typeof synthetic>):MarketEvidence {
 return {symbol:'QQQ',at:f.now,session:`QQQ:2026-10-26:${f.s.startedMs}`,flow:{signal:.4,coverage:80,at:f.now-1000,reason:'fixture'},cvd:{signal:.5,coverage:100,at:f.now-1000,reason:'fixture'}} as MarketEvidence;
}
const build=(f:ReturnType<typeof recorded>,active=true,e?:MarketEvidence)=>buildStructuralMapContext(f.s,'QQQ',active,f.now,f.input,e);

describe('recorded QQQ context: dated references, no forecast',()=>{
 it('preserves five real scale readings and shares one partial GEX reference',()=>{
  const f=recorded(),r=build(f);expect(r.price.map(c=>c.side)).toEqual(real.expected.sides);expect(r.price.every(c=>c.status==='available')).toBe(true);
  for(let i=0;i<5;i++){expect(r.price[i].atr).toBe(f.input.cards[i].atr);expect(r.price[i].lastClose).toBe(f.input.cards[i].lastClose);expect(r.price[i].closedAtMs).toBe(f.input.cards[i].endMs);}
  expect(r.options.status).toBe('partial');expect(r.options.profile?.rows).toHaveLength(76);expect(r.options.net).toBeCloseTo(-300979420.01492465,5);expect(r.options.countCoveragePct).toBe(80.46875);expect(r.options.riskCoveragePct).toBeNull();expect(r.options.priceDirectionInferred).toBe(false);
  expect(r.options.freshness).toBe('historical');expect(r.options.expiry).toBe('2026-10-26');expect(r.options.spot).toBe(704.88);expect(r.price[0].lastClose).toBe(704.39);expect(r.options.spotTimeMs).toBeNull();expect(r.options.oiTimeMs).toBeNull();
  expect(r.links).toHaveLength(5);expect(new Set(r.links.map(x=>x.contextId)).size).toBe(1);expect(r.links.every(x=>x.role==='context-only')).toBe(true);expect(r.probability).toBeNull();expect(r.directionScore).toBeNull();
  if(process.env.SNIPERS_STRUCTURAL_CONTEXT_AUDIT_DIR){
   const samples:number[]=[];for(let i=0;i<300;i++){const start=performance.now();build(f);samples.push(performance.now()-start);}samples.sort((a,b)=>a-b);
   writeFileSync(join(process.env.SNIPERS_STRUCTURAL_CONTEXT_AUDIT_DIR,'context-real-output.json'),JSON.stringify(r,null,2));
   writeFileSync(join(process.env.SNIPERS_STRUCTURAL_CONTEXT_AUDIT_DIR,'context-timing.json'),JSON.stringify({calls:samples.length,meanMs:samples.reduce((a,b)=>a+b,0)/samples.length,p95Ms:samples[Math.floor(samples.length*.95)],scope:'Pure join on frozen QQQ outputs; not full UI or live-provider acceptance'},null,2));
  }
 });
 it('identifies overlapping option populations without adding them',()=>{
  const r=build(recorded()),week=r.optionScopes.find(x=>x.mode==='WEEK')!,next=r.optionScopes.find(x=>x.mode==='NEXT4')!;
  expect(week.overlaps.find(x=>x.mode==='NEXT4')?.expirations).toEqual(next.expirations);
  expect(next.populationKeys.every(k=>week.populationKeys.includes(k))).toBe(true);
  expect(r.strikeFlow.cells).toBe(0);expect(r.strikeFlow.status).toBe('pending');expect(r.strikeFlow.receptionCoveragePct).toBeNull();expect(r.strikeFlow.directionSupportAllowed).toBe(false);
 });
 it('leaves absent activity pending and never copies five minutes into a price scale',()=>{
  const r=build(recorded());for(const e of Object.values(r.activity)){expect(e.status).toBe('pending');expect(e.signal).toBeNull();expect(e.assignableStructureIntervals).toEqual([]);}
 });
 it('has no Gexbot dependency and never mutates the input',()=>{
  const f=recorded(),before=structuredClone(f),a=build(f);Object.assign(f.s,{gexbotDominants:{positiveOi:1e99},gexbot:{netGex:1e100}});expect(build(f)).toEqual(a);delete (f.s as unknown as Record<string,unknown>).gexbotDominants;delete (f.s as unknown as Record<string,unknown>).gexbot;expect(f).toEqual(before);
 });
});

describe('identity, causal time and incomplete input',()=>{
 it.each(['foreign-frame','future-published','invalid-now','missing-published'] as const)('rejects %s across sources',kind=>{
  const f=synthetic();if(kind==='foreign-frame')f.s.symbol='SPY';if(kind==='future-published')f.s.publishedMs=f.now+1;if(kind==='invalid-now')f.now=NaN;if(kind==='missing-published')f.s.publishedMs=0;
  const r=build(f);expect(r.frame.status).toBe('rejected');expect(r.options.profile).toBeNull();expect(r.price.every(c=>c.status==='rejected')).toBe(true);expect(r.links).toEqual([]);
 });
 it.each(['symbol','publication','evaluation','duplicate-scale','missing-scale','future-close','future-pivot','unstable-atr','invalidated-direction'] as const)('quarantines structural %s',kind=>{
  const f=synthetic(),c=f.input.cards[0];if(kind==='symbol')f.input.symbol='SPY';if(kind==='publication')f.input.publishedMs--;if(kind==='evaluation')f.input.evaluatedMs--;if(kind==='duplicate-scale')f.input.cards[1].interval=15;if(kind==='missing-scale')f.input.cards=f.input.cards.slice(1);if(kind==='future-close')c.endMs=f.now+1;if(kind==='future-pivot')c.highs[1].confirmedMs=f.now+1;if(kind==='unstable-atr')c.seedDifference=.006;if(kind==='invalidated-direction')c.invalidated=true;
  const r=build(f);expect(r.price[0].status).toBe('rejected');expect(r.links.some(l=>l.interval===15)).toBe(false);expect(r.options.profile).not.toBeNull();
 });
 it('keeps historical or disconnected data explicitly historical',()=>{
  const f=synthetic();expect(build(f).options.freshness).toBe('current');expect(build(f,false).options.freshness).toBe('historical');expect(build(f,false).price.every(x=>x.freshness==='historical')).toBe(true);
  f.now+=15001;f.input.evaluatedMs=f.now;expect(build(f).frame.freshness).toBe('historical');expect(build(f).options.freshness).toBe('historical');
 });
 it('rejects values that postdate the publication even when they precede evaluation',()=>{
  const f=synthetic(),e=evidence(f);f.s.publishedMs-=1000;f.input.publishedMs=f.s.publishedMs;f.input.cards[0].endMs=f.now-500;e.flow.at=f.now-500;f.s.flow.heatmap!.asofMs=f.now-500;
  const r=build(f,true,e);expect(r.price[0].status).toBe('rejected');expect(r.activity.flow.status).toBe('rejected');expect(r.strikeFlow.status).toBe('rejected');
 });
 it('preserves absent scales as pending, not numeric zero',()=>{
  const f=synthetic();f.input.cards[0].side='SIN DATOS';const r=build(f);expect(r.price[0].lastClose).toBeNull();expect(r.price[0].status).toBe('pending');expect(r.links).toHaveLength(4);
 });
 it.each(['null-card','non-array-pivots','null-pivot'] as const)('quarantines malformed %s without throwing',kind=>{
  const f=synthetic();if(kind==='null-card')(f.input.cards as unknown[])[0]=null;if(kind==='non-array-pivots')Object.assign(f.input.cards[0],{highs:'AA'});if(kind==='null-pivot')(f.input.cards[0].highs as unknown[])[0]=null;expect(build(f).price[0].status).toBe('rejected');
 });
 it('ignores null horizon entries and rejects a missing activity component',()=>{
  const f=synthetic();(f.s.optionSnapshotStatus.horizons as unknown[]).push(null);const e=evidence(f);delete (e as unknown as Record<string,unknown>).flow;expect(build(f,true,e).activity.flow.status).toBe('rejected');expect(build(f).optionScopes).toHaveLength(1);
 });
 it.each(['rows','clock','expiry','ambiguous'] as const)('quarantines invalid options %s without changing structure',kind=>{
  const f=synthetic(),h=f.s.optionSnapshotStatus.horizons[0];if(kind==='rows')h.exposure.strikes.push({...h.exposure.strikes[0]});if(kind==='clock')h.observedMs=f.now+1;if(kind==='expiry'){h.mode='NEXT_SESSION';h.expirations=['2026-13-01'];}if(kind==='ambiguous')f.s.optionSnapshotStatus.horizons.push({...h});
  const r=build(f);expect(r.options.profile).toBeNull();expect(r.links).toEqual([]);expect(r.price.every(c=>c.status==='available')).toBe(true);
 });
 it('keeps the original exposure reader public and numerical result unchanged',()=>{
  const f=recorded(),v=readMassiveExposure(f.s,'QQQ',true,f.now);expect(build(f).options.profile).toEqual(v);
 });
 it('does not change structure when the gamma convention changes sign',()=>{
  const f=synthetic(),before=build(f),e=f.s.optionSnapshotStatus.horizons[0].exposure;
  for(const row of e.strikes)Object.assign(row,{callGex:Math.abs(row.netGex),putGex:0,netGex:Math.abs(row.netGex)});e.netGex=e.strikes.reduce((n,r)=>n+r.netGex,0);
  const after=build(f);expect(after.options.net).toBeGreaterThan(0);expect(after.price).toEqual(before.price);expect(after.directionScore).toBeNull();
 });
});

describe('activity is not structural-window coverage',()=>{
 it('does not treat complete premiums as a complete reception window',()=>{
  const f=synthetic(),r=build(f,true,evidence(f));expect(r.activity.flow.status).toBe('partial');expect(r.activity.flow.signal).toBe(.4);expect(r.activity.flow.temporalCoverage).toBe('not-proven');expect(r.activity.flow.assignableStructureIntervals).toEqual([]);
 });
 it('rejects a pressure reading from a different flow capture',()=>{
  const f=synthetic();f.s.flow.asof_ms=f.now-2000;expect(build(f,true,evidence(f)).activity.flow.status).toBe('rejected');
 });
 it.each(['symbol','session','evaluation','future-at','bad-signal','bad-coverage','stale-publication'] as const)('rejects activity %s',kind=>{
  const f=synthetic(),e=evidence(f);if(kind==='symbol')e.symbol='SPY';if(kind==='session')e.session+='other';if(kind==='evaluation')e.at--;if(kind==='future-at')e.flow.at=f.now+1;if(kind==='bad-signal')e.flow.signal=2;if(kind==='bad-coverage')e.flow.coverage=101;if(kind==='stale-publication'){f.s.publishedMs=f.now-3001;f.input.publishedMs=f.s.publishedMs;}
  expect(build(f,true,e).activity.flow.status).toBe('rejected');
 });
 it('accepts covered five-minute CVD only as immediate context',()=>{
  const f=synthetic(),e=evidence(f);f.s.cvd={symbol:'QQQ',date:'2026-10-26',at:f.now,available:true,recentFive:{startMs:f.now-301000,endMs:f.now-1000,complete:true,buy:75,sell:25,unknown:0,delta:50}} as LiveState['cvd'];
  let r=build(f,true,e);expect(r.activity.cvd.status).toBe('available');expect(r.activity.cvd.temporalCoverage).toBe('complete');expect(r.activity.cvd.assignableStructureIntervals).toEqual([]);
  f.s.cvd!.recentFive!.complete=false;expect(build(f,true,e).activity.cvd.status).toBe('rejected');f.s.cvd!.recentFive!.complete=true;f.s.cvd!.recentFive!.delta=-50;expect(build(f,true,e).activity.cvd.status).toBe('rejected');
 });
 it.each(['foreign','future','method'] as const)('quarantines strike-flow %s',kind=>{
  const f=synthetic(),h=f.s.flow.heatmap!;if(kind==='foreign')h.symbol='SPY';if(kind==='future')h.asofMs=f.now+1;if(kind==='method')h.method='unverified';expect(build(f).strikeFlow.status).toBe('rejected');expect(build(f).strikeFlow.directionSupportAllowed).toBe(false);
 });
});
