import {ChartSignalOutcomeMonitor,validChartSignalOutcomesReport,chartSignalOutcomesSemanticKey} from './chart-signal-outcomes';
import {expect,it} from 'vitest';
import {ChartSignalCoordinator,validChartSignalCoordinatorReport,type ChartSignalCoordinatorInput} from './chart-signal-coordinator';
import {buildChartSignalDiagnostics,chartSignalDiagnosticsSemanticKey} from './chart-signal-diagnostics';
import type {ChartSignalCoverageReport,ChartSignalCoverageKind} from './chart-signal-coverage';
import type {RouteOpportunitySource,FrozenRouteOpportunity} from './route-opportunity-source';
import type {Candle} from './model';
import fs from 'node:fs';import path from 'node:path';
const minute=60000,open=Date.parse('2026-10-26T09:30:00-04:00'),trigger=open+30*minute,now=trigger+200,started=open-60000,session=`QQQ:2026-10-26:${started}`;
const kinds:ChartSignalCoverageKind[]=['price-clock','closed-minute-continuity','stock-share-classification','option-contract-classification','option-premium-classification','option-premium-availability','stock-reception','option-reception'];
const bar=(start:number,close=100):Candle=>({timestampMs:start,open:100,high:Math.max(100.1,close),low:Math.min(99.9,close),close,volume:10});
function input(direction:1|-1=1,offset=0):ChartSignalCoordinatorInput{
 const trigger=open+30*minute+offset,now=trigger+200,key=offset?'episode-'+offset:'episode-1',published=now-50;
 const e:FrozenRouteOpportunity={version:'frozen-route-opportunity-v1',strategyId:'session-intraday-v1',sourceResultKey:key,sourceState:'confirmed',direction,origin:{symbol:'QQQ',session,publishedMs:published,startedMs:started,evaluatedMs:now,marketEvidenceKey:'source-market'},signal:{id:key+':confirmed',session,availableMs:now,evidenceKey:'source-evidence'},impulseBar:{startMs:trigger-4*minute,endMs:trigger-3*minute,close:100},triggerBar:{startMs:trigger-minute,endMs:trigger,close:100},continuationBar:{startMs:trigger-minute,endMs:trigger,close:100},anchor:{name:'session pivot',price:100,expiration:'2026-10-26',firstObservedMs:open,asofMs:open,source:'session-price'},target:{price:100+2*direction,name:'session target',expiration:'2026-10-26',firstObservedMs:open,asofMs:open,source:'session-price',selectedMs:trigger+100},stop:100-direction,expiry:{clock:'bar-start',elapsedMs:600000,lastAllowedBarStartMs:trigger+6*minute,comparison:'>'}};
 const current={state:'confirmed',resultState:'sustained' as const,reason:'source',key,direction,level:100,target:e.target.price,stop:e.stop,evaluatedMs:now,resultAt:now,lastClosedBar:e.triggerBar};
 const source:RouteOpportunitySource={version:'route-opportunity-source-v1',symbol:'QQQ',publishedMs:published,startedMs:started,at:now,market:{symbol:'QQQ',session,key:'source-market',at:now},strategies:[{strategyId:'important-level-route-v1',current:{...current,state:'sustained'},event:{...e,strategyId:'important-level-route-v1',sourceState:'sustained'}},{strategyId:'session-intraday-v1',current,event:e}]};
 const cov:ChartSignalCoverageReport={version:'chart-signal-coverage-v1',symbol:'QQQ',sessionKey:session,evaluatedMs:now,publishedMs:published,connectionStartedMs:started,reasonCodes:[],rows:kinds.map(kind=>({kind,unit:kind==='closed-minute-continuity'?'minutes':kind.includes('contract')?'contracts':kind.includes('premium')?'USD':kind.includes('reception')?'events':'shares',sourcePath:'synthetic.'+kind,numerator:80,denominator:100,percent:80,complete:true,status:'available',window:{startMs:published-300000,endMs:published,durationMs:300000,boundsBasis:'source-recent-five'},freshness:{sourceAtMs:published,sourceAgeMs:50,publishedAgeMs:50,publicationLagMs:0,lastEventMs:published,eventAgeMs:50},reasonCodes:[]})),feedDiagnostics:{stocks:null,options:null,optionCapture:null,flow:null,cvd:null},existingConsumer:{flow:null,cvd:{signal:direction*.5,coverage:80,at:published,reason:'synthetic'}}};
 const r={state:'sustained' as const,direction,level:100,target:e.target.price,stop:e.stop,at:now,reason:'source',key};
 const diagnostics=buildChartSignalDiagnostics({coverage:cov,important:{result:r,assessmentQuality:'aligned',cvd:{status:'aligned',label:'source',detail:'synthetic',at:published,coverage:80,observations:2},structure:null},intraday:{state:'confirmed',result:r,side:direction===1?'CALL':'PUT',blockCodes:[]}});
 return {symbol:'QQQ',at:now,publishedMs:published,startedMs:started,active:true,calendar:{date:'2026-10-26',openMs:open,closeMs:open+390*minute},bars:Array.from({length:4},(_,i)=>bar(trigger-(4-i)*minute)),source,diagnostics};
}
function later(original:ChartSignalCoordinatorInput,elapsed:number,extra:Candle[]=[]):ChartSignalCoordinatorInput{
 const at=original.at+elapsed,published=original.publishedMs!+elapsed,s=original.source!,d=original.diagnostics!;
 return {...original,at,publishedMs:published,bars:[...original.bars,...extra],source:{...s,at,publishedMs:published,market:{...s.market,at},strategies:s.strategies.map(v=>({...v,current:{...v.current,evaluatedMs:at}}))},diagnostics:{...d,evaluatedMs:at,publishedMs:published,coverage:{...d.coverage,evaluatedMs:at,publishedMs:published}}};
}
const own=(r:ReturnType<ChartSignalCoordinator['observe']>)=>r.signals.find(v=>v.strategyId==='session-intraday-v1')!;
it.each([1,-1]as const)('copies only source confirmation and signed immutable underlying geometry for %d',direction=>{
 const c=new ChartSignalCoordinator(),i=input(direction),before=JSON.stringify(i),r=c.observe(i),s=own(r);
 expect(s.state).toBe('confirmed');expect(s.ticket?.geometry).toMatchObject({direction,risk:1,reward:2,rewardRisk:2});expect(s.ticket?.entryReference?.kind).toBe('observed-underlying-reference-not-fill');
 expect(r.signals[0].state).toBe('candidate');expect(r.signals[0].ticket).toBeNull();expect(validChartSignalCoordinatorReport(r)).toBe(true);expect(JSON.stringify(i)).toBe(before);expect(Object.isFrozen(s.ticket)).toBe(true);
 expect(r).toMatchObject({entryAuthorized:false,executionEnabled:false,markersEnabled:false,probability:null,fill:null});
 if(process.env.SNIPERS_STAGE3_PROTOCOL_ROOT)fs.writeFileSync(path.join(process.env.SNIPERS_STAGE3_PROTOCOL_ROOT,`protocol-${direction}.json`),JSON.stringify({synthetic:true,input:i,report:r},null,2));
});
it('shares one read-only event between quick/detailed redraws and never turns GEX into another vote',()=>{
 const c=new ChartSignalCoordinator(),i=input(),r=c.observe(i);expect(c.snapshot('QQQ',now)).toBe(r);expect(c.snapshot('QQQ',now+1)).toBe(r);expect(c.snapshot('SPY',now)).toBeUndefined();expect(c.snapshot('QQQ',now+3001)).toBeUndefined();
 expect(c.observe(i).semanticKey).toBe(r.semanticKey);const next=c.observe(later(i,1));expect(own(next).id).toBe(own(r).id);expect(own(next).ticket).toBe(own(r).ticket);expect(next.evidencePolicy).toMatchObject({gexSelection:'not-an-input',independentVotes:null,receptionPercent:null});expect(next.hypotheses[1].role).toBe('separate-context-not-an-extra-vote');
});
it.each([1,-1]as const)('resolves a later contiguous close to the immutable target for %d',direction=>{
 const c=new ChartSignalCoordinator(),i=input(direction),r=c.observe(i),n=c.observe(later(i,minute,[bar(trigger,100+2.1*direction)]));expect(own(n).state).toBe('reached');expect(own(n).ticket).toBe(own(r).ticket);expect(own(n).lastResolvedCloseMs).toBe(trigger+minute);
});
it('does not backfill a confirmed source event or use missing optional option coverage as a new veto',()=>{
 const i=input(),c=new ChartSignalCoordinator();expect(own(c.observe(later(i,1))).ticket).toBeNull();
 const d=structuredClone(i.diagnostics!);(d.coverage as ChartSignalCoverageReport).rows=d.coverage.rows.map(v=>v.kind.startsWith('option-')?{...v,status:'unavailable',complete:null,numerator:null,denominator:null,percent:null}:v);
 const normalized={...d,semanticKey:chartSignalDiagnosticsSemanticKey(d)};
 expect(own(new ChartSignalCoordinator().observe({...i,diagnostics:normalized})).state).toBe('confirmed');
});
it.each(['partial-cvd','low-cvd','gap','foreign','closed','future-target']as const)('fails closed for %s without modifying the source',kind=>{
 const i=structuredClone(input());
 if(kind==='partial-cvd'||kind==='low-cvd')(i.diagnostics!.coverage as ChartSignalCoverageReport).rows=i.diagnostics!.coverage.rows.map(v=>v.kind==='stock-share-classification'?{...v,complete:kind!=='partial-cvd',percent:kind==='low-cvd'?32:80}:v);
 if(kind==='gap')i.bars=i.bars.slice(1);
 if(kind==='foreign')i.source={...i.source!,symbol:'SPY'};
 if(kind==='closed')i.calendar={...i.calendar!,closeMs:now-1};
 if(kind==='future-target')i.source!.strategies[1].event!.target.firstObservedMs=now+1;
 i.diagnostics={...i.diagnostics!,semanticKey:chartSignalDiagnosticsSemanticKey(i.diagnostics!)};
 const before=JSON.stringify(i),r=new ChartSignalCoordinator().observe(i);expect(own(r).ticket).toBeNull();expect(JSON.stringify(i)).toBe(before);
});
it('never skips an unknown earlier close to manufacture a later outcome',()=>{
 const c=new ChartSignalCoordinator(),i=input();c.observe(i);const n=c.observe(later(i,2*minute,[bar(trigger+minute,103)]));expect(own(n).state).toBe('confirmed');expect(own(n).reasonCodes).toContain('resolution-close-unavailable');
});
it('records a resolved bar revision without rewriting an already reached ticket or result',()=>{
 const c=new ChartSignalCoordinator(),i=input();c.observe(i);const n=later(i,minute,[bar(trigger,103)]),reached=c.observe(n);const revised={...later(n,1),bars:[...i.bars,bar(trigger,98)]};const r=c.observe(revised);expect(own(r).state).toBe('reached');expect(own(r).revisionObserved).toBe(true);expect(own(r).ticket).toBe(own(reached).ticket);
});
it('expires only after all eligible one-minute closes have been observed, with the original deadline',()=>{
 const c=new ChartSignalCoordinator(),i=input();const first=c.observe(i),deadline=own(first).ticket!.deadlineMs;
 const steps=(deadline-trigger)/minute+1,n=c.observe(later(i,steps*minute,Array.from({length:steps},(_,j)=>bar(trigger+j*minute))));expect(own(n).state).toBe('expired');expect(own(n).ticket?.deadlineMs).toBe(deadline);
});
it('preserves private identity after a backward observer instead of admitting a new event',()=>{
 const c=new ChartSignalCoordinator(),i=input(),first=c.observe(i);expect(c.observe(later(i,-1)).status).toBe('waiting');expect(own(c.observe(later(i,1))).ticket).toBe(own(first).ticket);
});
it('rejects corrupted geometry, negative prices, execution flags and a forged legacy confirmation',()=>{
 const r=new ChartSignalCoordinator().observe(input());
 for(const change of [(r:any)=>r.executionEnabled=true,(r:any)=>r.signals[1].ticket.geometry.reward=99,(r:any)=>{r.signals[1].ticket.entryReference.price=-100;r.signals[1].ticket.target.price=-98;r.signals[1].ticket.invalidation.price=-101;},(r:any)=>r.signals[1].strategyId='important-level-route-v1']){const x=structuredClone(r);change(x);expect(validChartSignalCoordinatorReport(x)).toBe(false);}
});
it('rejects a mismatched calendar date interval and a revised first trigger close',()=>{
 const i=input();expect(own(new ChartSignalCoordinator().observe({...i,calendar:{...i.calendar!,openMs:open-86400000}})).ticket).toBeNull();const s=structuredClone(i.source!);s.strategies[1].event!.triggerBar.close=101;expect(own(new ChartSignalCoordinator().observe({...i,source:s})).reasonCodes).toContain('source-price-proof-incompatible');
});

it('independent outcomes ignore source terminal labels and consume only subsequent price evidence',()=>{const c=new ChartSignalCoordinator(),m=new ChartSignalOutcomeMonitor(),i=input(),first=c.observe(i);const a=m.observe(first,i.bars,i.symbol,i.at,i.publishedMs);expect(a.rows).toHaveLength(1);expect(a.rows[0].outcome).toBe('pending');expect(validChartSignalOutcomesReport(a)).toBe(true);const n=later(i,minute,[bar(trigger,102)]),common=c.observe(n),b=m.observe(common,n.bars,n.symbol,n.at,n.publishedMs);expect(b.rows[0].outcome).toBe('target');expect(validChartSignalOutcomesReport(b)).toBe(true);const fake={...structuredClone(common),signals:common.signals.map((v,index)=>index===1?{...v,state:'invalidated' as const}:v)};expect(m.observe(fake,n.bars,n.symbol,n.at,n.publishedMs).rows[0].outcome).toBe('target');expect(m.snapshot(i.symbol,n.at)).toBeDefined();});
it('does not invent an old source ticket when a monitor starts after confirmation',()=>{const c=new ChartSignalCoordinator(),i=input(),first=c.observe(i),m=new ChartSignalOutcomeMonitor(),laterInput=later(i,1),common=c.observe(laterInput),r=m.observe(common,laterInput.bars,i.symbol,laterInput.at,laterInput.publishedMs);expect(first.signals[1].ticket).toBeTruthy();expect(r.rows).toHaveLength(0);expect(r.reasonCodes).toContain('older-ticket-not-backfilled');});

it('quarantines source revisions and rejects forged persisted excursion values',()=>{const c=new ChartSignalCoordinator(),m=new ChartSignalOutcomeMonitor(),i=input();m.observe(c.observe(i),i.bars,i.symbol,i.at,i.publishedMs);const n=later(i,minute,[bar(trigger,102)]),common=c.observe(n),good=m.observe(common,n.bars,n.symbol,n.at,n.publishedMs);const rows=good.rows.map(v=>({...v,excursions:{...v.excursions,mfe:999}})),changed={...good,rows},forged={...changed,semanticKey:chartSignalOutcomesSemanticKey(changed)};expect(validChartSignalOutcomesReport(forged)).toBe(false);const revised={...common,signals:common.signals.map((v,index)=>index===1?{...v,revisionObserved:true}:v)};revised.semanticKey=JSON.stringify(revised.signals.map(v=>[v.id,v.strategyId,v.sourcePhase,v.state,v.ticket?.provenance.sourceFingerprint??null,v.reasonCodes,v.lastAudit?.reasonCode,v.revisionObserved]));const report=m.observe(revised,n.bars,n.symbol,n.at,n.publishedMs);expect(report.rows[0].outcome).toBe('censored');expect(report.rows[0].firstObservedOutcome?.kind).toBe('target');expect(report.rows[0].issues).toContain('source-evidence-revised');});

it('retains censored history but admits the next original episode after the old fixed deadline',()=>{const c=new ChartSignalCoordinator(),m=new ChartSignalOutcomeMonitor(),first=input(),a=c.observe(first);m.observe(a,first.bars,first.symbol,first.at,first.publishedMs);const gap=later(first,2*minute,[bar(trigger+minute,103)]),blocked=c.observe(gap);m.observe(blocked,gap.bars,gap.symbol,gap.at,gap.publishedMs);expect(own(blocked).state).toBe('confirmed');const next=input(1,8*minute),r=c.observe(next),report=m.observe(r,next.bars,next.symbol,next.at,next.publishedMs);expect(own(r).id).not.toBe(own(a).id);expect(own(r).state).toBe('confirmed');expect(report.rows).toHaveLength(2);expect(report.rows[0].outcome).toBe('censored');expect(report.rows[0].firstObservedOutcome).toBeNull();expect(report.rows[1].outcome).toBe('pending');expect(validChartSignalOutcomesReport(report)).toBe(true);});
