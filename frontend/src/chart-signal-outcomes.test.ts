import fs from 'node:fs';import path from 'node:path';
import {expect,it} from 'vitest';
import {ChartSignalOutcomeTracker,ChartSignalOutcomeMonitor,replayChartSignalOutcome,validChartSignalOutcomesReport,chartSignalOutcomesSemanticKey,type OutcomeFrame} from './chart-signal-outcomes';
import type {ChartSignalTicket} from './chart-signal-contract';
import {ChartSignalCoordinator} from './chart-signal-coordinator';
const minute=60000,open=Date.parse('2026-10-27T09:30:00-04:00'),trigger=open+30*minute,known=trigger+200;
export function outcomeTicket(direction:1|-1=1):ChartSignalTicket{
 const strategyId='session-intraday-v1',sessionKey='QQQ:2026-10-27:1793104831065';
 return {version:'chart-signal-contract-v1',identity:{id:'test-ticket-'+direction,symbol:'QQQ',sessionKey,strategyId,sourceEventKey:'synthetic-episode'},side:direction===1?'CALL':'PUT',session:{openMs:open,closeMs:open+390*minute},sourceDataMs:known-50,publishedMs:known-50,availableMs:known,confirmedAtMs:known,impulseCloseMs:trigger-3*minute,triggerCloseMs:trigger,deadlineMs:trigger+7*minute,
 source:{kind:'verified-sequence',state:'confirmed',support:'complete'},entryReference:{price:100,atMs:trigger,kind:'observed-underlying-reference-not-fill'},target:{price:100+2*direction,name:'previously observed target',firstAvailableMs:open,selectedMs:trigger+100},invalidation:{price:100-direction,method:'one-minute-close'},quality:{status:'qualified',reasonCodes:[]},provenance:{engine:strategyId,engineVersion:'synthetic-outcome-protocol',frameId:'original-frame',evidenceKey:'original-evidence',sourceFingerprint:'frozen-source',coverage:[{name:'source-sequence-closed-minutes',unit:'minutes',status:'available',numerator:4,denominator:4}]},reasonCodes:[],geometry:{direction,risk:1,reward:2,rewardRisk:2,unit:'underlying-price-units'},resolution:'closed-one-minute-bar',deadlineRule:'min(impulse-close-plus-10m,session-close);exclusive-new-confirmation',entryAuthorized:false,executionEnabled:false,probability:null,fill:null};
}
function frame(t:ChartSignalTicket,elapsed:number,closes:number[],options:{high?:number;low?:number;offset?:number}={}):OutcomeFrame{
 const at=known+elapsed*minute;
 return {symbol:t.identity.symbol,sessionKey:t.identity.sessionKey,evaluatedMs:at,publishedMs:at-50,bars:closes.map((close,j)=>({timestampMs:trigger+((options.offset??0)+j)*minute,open:100,high:options.high??Math.max(100.1,close),low:options.low??Math.min(99.9,close),close}))};
}
it.each([1,-1]as const)('independently measures signed close excursions and target for %d',direction=>{
 const ticket=outcomeTicket(direction),before=JSON.stringify(ticket),tracker=new ChartSignalOutcomeTracker(ticket);
 const row=tracker.observe(frame(ticket,3,[100+direction*.4,100-direction*.2,100+direction*2]));
 expect(row.outcome).toBe('target');expect(row.firstObservedOutcome).toEqual({kind:'target',closeMs:trigger+3*minute,availableMs:known+3*minute});expect(row.excursions).toMatchObject({samples:3,mfe:2,complete:true,mfeR:2});expect(row.excursions.mae).toBeCloseTo(.2,12);expect(row.latency.outcomeObservationDelayMs).toBe(200);
 expect(row.ranges.excludedPreKnownBars).toBe(1);expect(row.ranges.bars).toBe(2);expect(row.executionEnabled).toBe(false);expect(row.fill).toBeNull();expect(row.pnl).toBeNull();expect(JSON.stringify(ticket)).toBe(before);expect(Object.isFrozen(row.samples)).toBe(true);
});
it.each([1,-1]as const)('uses strict stop and inclusive target without OHLC filling for %d',d=>{
 const ticket=outcomeTicket(d),tracker=new ChartSignalOutcomeTracker(ticket);
 expect(tracker.observe(frame(ticket,1,[100-d])).outcome).toBe('pending');
 const row=tracker.observe(frame(ticket,2,[100-d,100-1.1*d]));expect(row.outcome).toBe('invalidation');expect(row.firstObservedOutcome?.closeMs).toBe(trigger+2*minute);
});
it('does not use first partial bar high/low or assume an intrabar order when both prices are crossed',()=>{
 const t=outcomeTicket(),r=new ChartSignalOutcomeTracker(t).observe(frame(t,2,[100.1,100.1],{high:103,low:98}));
 expect(r.outcome).toBe('pending');expect(r.excursions.mfe).toBeCloseTo(.1);expect(r.ranges).toMatchObject({bars:1,excludedPreKnownBars:1,mfe:3,mae:2,bothLevelsTouchedBars:1,fillOrderKnown:false});
});
it('unknown earlier close prevents a later target and records censorship after the deadline',()=>{
 const t=outcomeTicket(),tracker=new ChartSignalOutcomeTracker(t);let r=tracker.observe(frame(t,2,[103],{offset:1}));expect(r.outcome).toBe('pending');expect(r.excursions.mfe).toBeNull();expect(r.coverage.firstMissingCloseMs).toBe(trigger+minute);
 r=tracker.observe(frame(t,8,[103],{offset:1}));expect(r.outcome).toBe('censored');expect(r.firstObservedOutcome).toBeNull();expect(r.coverage.complete).toBe(false);
});
it('late repair can make a terminal label knowable only at repair receipt, never at its old close',()=>{
 const t=outcomeTicket(),tracker=new ChartSignalOutcomeTracker(t);tracker.observe(frame(t,2,[103],{offset:1}));const r=tracker.observe(frame(t,3,[100.2,103]));
 expect(r.outcome).toBe('target');expect(r.firstObservedOutcome?.closeMs).toBe(trigger+2*minute);expect(r.firstObservedOutcome?.availableMs).toBe(known+3*minute);expect(r.latency.outcomeObservationDelayMs).toBe(minute+200);
});
it('preserves the first observed terminal and its original data when a later bar is replaced',()=>{
 const t=outcomeTicket(),tracker=new ChartSignalOutcomeTracker(t),first=tracker.observe(frame(t,1,[102]));const r=tracker.observe(frame(t,2,[98]));
 expect(r.outcome).toBe('censored');expect(r.firstObservedOutcome).toEqual(first.firstObservedOutcome);expect(r.samples[0].close).toBe(102);expect(r.revisions.count).toBe(1);expect(r.revisions.examples[0].replacement.close).toBe(98);expect(r.excursions.complete).toBe(false);
 expect(tracker.observe(frame(t,3,[98])).revisions.count).toBe(1);
});
it('conflicting duplicate bars in one publication have an ambiguous outcome',()=>{
 const t=outcomeTicket(),f=frame(t,1,[102]),r=new ChartSignalOutcomeTracker(t).observe({...f,bars:[...f.bars,{...f.bars[0],close:98,high:103,low:97}]});expect(r.outcome).toBe('ambiguous');expect(r.revisions.conflictExamples[0].original.close).toBe(102);expect(r.revisions.conflictExamples[0].replacement.close).toBe(98);expect(r.firstObservedOutcome).toBeNull();expect(r.excursions.samples).toBe(0);
});
it('ignores future bars and does not accept backward, foreign or oversized frames',()=>{
 const t=outcomeTicket(),tracker=new ChartSignalOutcomeTracker(t),f=frame(t,1,[100.1,103]);expect(tracker.observe(f).outcome).toBe('pending');
 for(const bad of [{...f,evaluatedMs:known-1},{...f,publishedMs:f.evaluatedMs+1},{...f,symbol:'SPY'},{...f,bars:Array.from({length:4097},()=>f.bars[0])}])tracker.observe(bad);
 const r=tracker.snapshot();expect(r.rejectedFrames).toBe(4);expect(r.samples).toHaveLength(1);expect(r.firstObservedOutcome).toBeNull();
});
it('does not turn malformed OHLC into a valid close observation',()=>{
 const t=outcomeTicket(),f=frame(t,1,[102]);const r=new ChartSignalOutcomeTracker(t).observe({...f,bars:[{...f.bars[0],high:101}]});expect(r.samples).toHaveLength(0);expect(r.issues).toContain('invalid-bar-excluded');
});
it('supports close-only original source evidence with range metrics unavailable',()=>{
 const t=outcomeTicket(),f=frame(t,2,[100.1,102]),r=new ChartSignalOutcomeTracker(t).observe({...f,bars:f.bars.map(({timestampMs,close})=>({timestampMs,close}))});
 expect(r.outcome).toBe('target');expect(r.ranges.mfe).toBeNull();expect(r.ranges.mae).toBeNull();expect(r.ranges.missingOhlcBars).toBe(1);
});
it('uses deadline only after its last eligible close is known and cannot win on a later close',()=>{
 const t=outcomeTicket(),tracker=new ChartSignalOutcomeTracker(t),closes=Array.from({length:7},()=>100.1),at=t.deadlineMs;
 const f=frame(t,7,closes);expect(tracker.observe({...f,evaluatedMs:at,publishedMs:at}).outcome).toBe('pending');
 const r=tracker.observe(frame(t,8,[...closes,103]));expect(r.outcome).toBe('deadline');expect(r.firstObservedOutcome?.closeMs).toBe(t.deadlineMs);expect(r.samples).toHaveLength(7);
});
it('the exact deadline close can hit a target even when the bar is reported later',()=>{
 const t=outcomeTicket(),r=new ChartSignalOutcomeTracker(t).observe(frame(t,9,[100,100,100,100,100,100,102]));expect(r.outcome).toBe('target');expect(r.firstObservedOutcome?.closeMs).toBe(t.deadlineMs);expect(r.latency.outcomeObservationDelayMs).toBe(2*minute+200);
});
it('EOF and epoch replacement censor open scenarios without inventing expiry or changing terminal results',()=>{
 const t=outcomeTicket(),tracker=new ChartSignalOutcomeTracker(t);tracker.observe(frame(t,1,[100.1]));expect(tracker.censor('session-epoch-ended').outcome).toBe('censored');
 const report=replayChartSignalOutcome(t,[frame(t,1,[100.1])],{sourceId:'synthetic-test',asOfMs:known+minute,ended:true});expect(report.outcome.outcome).toBe('censored');expect(report.sourceTerminalStateUsed).toBe(false);
 const closed=replayChartSignalOutcome(t,[frame(t,1,[102])],{sourceId:'synthetic-test',asOfMs:known+minute,ended:true});expect(closed.outcome.outcome).toBe('target');
});
it('rejects noncausal manifests and geometry rather than inventing entry or target prices',()=>{
 const t=outcomeTicket();expect(()=>replayChartSignalOutcome(t,[frame(t,1,[102])],{sourceId:'test',asOfMs:known,ended:true})).toThrow('after-asof');
 for(const change of [(v:ChartSignalTicket)=>v.geometry.reward=99,(v:ChartSignalTicket)=>v.target!.firstAvailableMs=known+1,(v:ChartSignalTicket)=>v.executionEnabled=true as false,(v:ChartSignalTicket)=>v.confirmedAtMs=v.deadlineMs]){const v=structuredClone(t);change(v);expect(()=>new ChartSignalOutcomeTracker(v)).toThrow();}
});
it('no existing admitted signal means no new outcome, and metadata clocks alone do not create state writes',()=>{
 const common=new ChartSignalCoordinator().observe({symbol:'QQQ',at:known,publishedMs:known-50,startedMs:open,active:true,calendar:null,bars:[],source:undefined,diagnostics:undefined}),m=new ChartSignalOutcomeMonitor(),r=m.observe(common,[],'QQQ',known,known-50);
 expect(r.rows).toHaveLength(0);expect(validChartSignalOutcomesReport(r)).toBe(true);expect(m.snapshot('QQQ',known)).toBe(r);expect(m.snapshot('SPY',known)).toBeUndefined();expect(m.snapshot('QQQ',known+3001)).toBeUndefined();expect(m.observe(undefined,[],'QQQ',known+1,known-49).semanticKey).toBe(r.semanticKey);
 const changed={...structuredClone(r),executionEnabled:true as false};const corrupted={...changed,semanticKey:chartSignalOutcomesSemanticKey(changed)};expect(validChartSignalOutcomesReport(corrupted)).toBe(false);
});

it('exports declared synthetic causal paths for independent replay arithmetic',()=>{
 const t=outcomeTicket(),put=outcomeTicket(-1),cases=[
  {id:'call-target',ticket:t,frames:[frame(t,1,[100.4]),frame(t,3,[100.4,99.8,102])],ended:false,expected:'target'},
  {id:'put-target',ticket:put,frames:[frame(put,3,[99.6,100.2,98])],ended:false,expected:'target'},
  {id:'strict-stop',ticket:t,frames:[frame(t,1,[99]),frame(t,2,[99,98.9])],ended:false,expected:'invalidation'},
  {id:'deadline',ticket:t,frames:[frame(t,8,[100.1,100.2,100.3,100.2,100.1,100,100.1,103])],ended:false,expected:'deadline'},
  {id:'gap-censored',ticket:t,frames:[frame(t,8,[103],{offset:1})],ended:false,expected:'censored'},
  {id:'late-repair',ticket:t,frames:[frame(t,2,[103],{offset:1}),frame(t,3,[100.2,103])],ended:false,expected:'target'},
  {id:'revision',ticket:t,frames:[frame(t,1,[102]),frame(t,2,[98])],ended:false,expected:'censored'},
  {id:'range-both-no-fill',ticket:t,frames:[frame(t,2,[100.1,100.2],{high:103,low:98})],ended:true,expected:'censored'},
  {id:'conflict',ticket:t,frames:[{...frame(t,1,[102]),bars:[...frame(t,1,[102]).bars,...frame(t,1,[98]).bars]}],ended:false,expected:'ambiguous'},
 ];
 const outputs=cases.map(c=>{const asOfMs=c.frames.at(-1)!.evaluatedMs,report=replayChartSignalOutcome(c.ticket,c.frames,{sourceId:'explicit-synthetic:'+c.id,asOfMs,ended:c.ended});expect(report.outcome.outcome).toBe(c.expected);return {...c,report};});
 const root=process.env.SNIPERS_STAGE4_PROTOCOL_ROOT;if(root){fs.mkdirSync(root,{recursive:true});fs.writeFileSync(path.join(root,(process.env.SNIPERS_STAGE4_SCOPE??'native')+'-protocol-suite.json'),JSON.stringify({synthetic:true,scope:'Software scenarios only. No original real source confirmation or financial acceptance.',cases:outputs},null,2));}
});
