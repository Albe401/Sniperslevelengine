import type {ChartSignalTicket} from './chart-signal-contract';
import {validChartSignalCoordinatorReport,type ChartSignalCoordinatorReport} from './chart-signal-coordinator';

/** Measurement conventions for research, never orders, fills, forecasts or calibrated probabilities. */
export const CHART_SIGNAL_OUTCOME_POLICY=Object.freeze({version:'chart-signal-outcomes-v1',resolution:'closed-one-minute-bar',targetComparison:'inclusive',invalidationComparison:'strict',maxBarsPerFrame:4096,maxSamples:10,maxRevisionExamples:3,maxRevisionKeys:32,maxEpisodes:4,maxReportBytes:16384,markersEnabled:false,executionEnabled:false} as const);
type Kind='pending'|'target'|'invalidation'|'deadline'|'censored'|'ambiguous';
export interface OutcomeBar {timestampMs:number;close:number;open?:number|null;high?:number|null;low?:number|null}
export interface OutcomeFrame {symbol:string;sessionKey:string;evaluatedMs:number;publishedMs:number;bars:readonly OutcomeBar[]}
export interface OutcomeSample {startMs:number;closeMs:number;close:number;open:number|null;high:number|null;low:number|null;observedAtMs:number;publishedMs:number}
export interface OutcomeReference {side:'CALL'|'PUT';entry:number;target:number;invalidation:number;risk:number;reward:number;rewardRisk:number;knownAtMs:number;triggerCloseMs:number;publishedMs:number;deadlineMs:number;sessionOpenMs:number;sessionCloseMs:number;firstEligibleCloseMs:number}
export interface OutcomeRow {
 id:string;symbol:string;sessionKey:string;ticketKey:string;reference:OutcomeReference;asOfMs:number;
 outcome:Kind;firstObservedOutcome:{kind:'target'|'invalidation'|'deadline';closeMs:number;availableMs:number}|null;
 coverage:{expectedCloses:number;observedCloses:number;contiguousCloses:number;firstMissingCloseMs:number|null;completeThroughCloseMs:number|null;complete:boolean};
 excursions:{basis:'post-confirmation-one-minute-closes';unit:'underlying-price-units';samples:number;throughCloseMs:number|null;mfe:number|null;mae:number|null;mfeR:number|null;maeR:number|null;complete:boolean};
 ranges:{basis:'fully-after-confirmation-ohlc-subset';unit:'underlying-price-units';bars:number;excludedPreKnownBars:number;missingOhlcBars:number;mfe:number|null;mae:number|null;bothLevelsTouchedBars:number;fillOrderKnown:false};
 latency:{triggerToConfirmationMs:number;publicationToConfirmationMs:number;outcomeObservationDelayMs:number|null};
 samples:readonly OutcomeSample[];revisions:{count:number;truncated:boolean;conflictingStarts:readonly number[];conflictExamples:readonly {observedAtMs:number;original:OutcomeSample;replacement:OutcomeSample}[];examples:readonly {observedAtMs:number;original:OutcomeSample;replacement:OutcomeSample}[]};
 rejectedFrames:number;issues:readonly string[];entryAuthorized:false;executionEnabled:false;probability:null;fill:null;pnl:null;
}
export interface ChartSignalOutcomesReport {
 version:typeof CHART_SIGNAL_OUTCOME_POLICY.version;mode:'shadow';symbol:string;sessionKey:string|null;evaluatedMs:number|null;publishedMs:number|null;
 status:'observed'|'waiting';scope:'current-and-recent-observed-episodes';rows:readonly Readonly<OutcomeRow>[];
 droppedEpisodes:number;reasonCodes:readonly string[];semanticKey:string;entryAuthorized:false;executionEnabled:false;markersEnabled:false;probability:null;fill:null;pnl:null;
}
const minute=60000;
const clock=(v:unknown):v is number=>typeof v==='number'&&Number.isSafeInteger(v)&&v>0&&v<=8640000000000000;
const positive=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v)&&v>0;
const text=(v:unknown):v is string=>typeof v==='string'&&v.length>0&&v.length<=4096;
const terminal=(k:Kind)=>k==='target'||k==='invalidation'||k==='deadline';
const encoder=new TextEncoder();
function freeze<T>(v:T):Readonly<T>{if(v&&typeof v==='object'&&!Object.isFrozen(v)){Object.values(v).forEach(freeze);Object.freeze(v);}return v;}
const canonical=(v:unknown):string=>Array.isArray(v)?'['+v.map(canonical).join(',')+']':v&&typeof v==='object'?'{'+Object.entries(v).sort(([a],[b])=>a<b?-1:a>b?1:0).map(([k,x])=>JSON.stringify(k)+':'+canonical(x)).join(',')+'}':JSON.stringify(v)??'undefined';
/** Separate outcome identity; no call to the lifecycle reducer or its terminal state. */
export function chartSignalOutcomeTicketKey(t:Readonly<ChartSignalTicket>):string{return canonical([t.version,t.identity,t.side,t.session,t.sourceDataMs,t.publishedMs,t.availableMs,t.confirmedAtMs,t.impulseCloseMs,t.triggerCloseMs,t.deadlineMs,t.entryReference,t.target,t.invalidation,t.geometry,t.provenance]);}
function reference(t:Readonly<ChartSignalTicket>):OutcomeReference {
 const d=t.side==='CALL'?1:t.side==='PUT'?-1:0,e=t.entryReference,p=t.target,i=t.invalidation,known=Math.max(t.availableMs,t.confirmedAtMs);
 if(t.version!=='chart-signal-contract-v1'||!d||!e||!p||!i||![e.price,p.price,i.price].every(positive)||![t.sourceDataMs,t.publishedMs,t.availableMs,t.confirmedAtMs,t.triggerCloseMs,t.impulseCloseMs,t.deadlineMs,t.session?.openMs,t.session?.closeMs].every(clock)||t.sourceDataMs>t.publishedMs||t.publishedMs>t.availableMs||t.availableMs>t.confirmedAtMs||t.triggerCloseMs>t.sourceDataMs||known>=t.deadlineMs||t.impulseCloseMs>=t.triggerCloseMs||t.impulseCloseMs<t.session.openMs+minute||t.session.openMs>=t.session.closeMs||t.deadlineMs!==Math.min(t.impulseCloseMs+10*minute,t.session.closeMs)||(t.triggerCloseMs-t.session.openMs)%minute!==0||(t.impulseCloseMs-t.session.openMs)%minute!==0||t.entryAuthorized!==false||t.executionEnabled!==false||t.fill!==null||t.probability!==null||t.resolution!=='closed-one-minute-bar'||i.method!=='one-minute-close'||e.kind!=='observed-underlying-reference-not-fill'||!clock(e.atMs)||e.atMs<t.triggerCloseMs||e.atMs>t.sourceDataMs||!clock(p.firstAvailableMs)||p.firstAvailableMs>t.triggerCloseMs||!clock(p.selectedMs)||p.selectedMs<p.firstAvailableMs||p.selectedMs>t.availableMs)throw Error('invalid-outcome-ticket');
 const risk=d*(e.price-i.price),reward=d*(p.price-e.price);
 if(!positive(risk)||!positive(reward)||!positive(reward/risk)||t.geometry?.direction!==d||t.geometry.risk!==risk||t.geometry.reward!==reward||t.geometry.rewardRisk!==reward/risk||t.geometry.unit!=='underlying-price-units'||!text(t.identity?.id)||!text(t.identity.symbol)||!text(t.identity.sessionKey)||t.source?.kind!=='verified-sequence'||t.source.support!=='complete'||t.quality?.status!=='qualified')throw Error('invalid-outcome-geometry');
 return {side:t.side,entry:e.price,target:p.price,invalidation:i.price,risk,reward,rewardRisk:reward/risk,knownAtMs:known,triggerCloseMs:t.triggerCloseMs,publishedMs:t.publishedMs,deadlineMs:t.deadlineMs,sessionOpenMs:t.session.openMs,sessionCloseMs:t.session.closeMs,firstEligibleCloseMs:t.session.openMs+(Math.floor((known-t.session.openMs)/minute)+1)*minute};
}
function sample(bar:OutcomeBar,frame:OutcomeFrame,r:OutcomeReference):OutcomeSample|null {
 if(!bar||!clock(bar.timestampMs)||!positive(bar.close)||(bar.timestampMs-r.sessionOpenMs)%minute!==0||bar.timestampMs<r.sessionOpenMs||bar.timestampMs+minute>Math.min(frame.publishedMs,r.deadlineMs))return null;
 const o=bar.open??null,h=bar.high??null,l=bar.low??null,empty=o===null&&h===null&&l===null;
 if(!empty&&(!positive(o)||!positive(h)||!positive(l)||h<Math.max(o,bar.close)||l>Math.min(o,bar.close)||l>h))return null;
 return {startMs:bar.timestampMs,closeMs:bar.timestampMs+minute,close:bar.close,open:o,high:h,low:l,observedAtMs:frame.evaluatedMs,publishedMs:frame.publishedMs};
}
const barKey=(b:OutcomeSample)=>JSON.stringify([b.startMs,b.close,b.open,b.high,b.low]);

/** Signed close-path measurement only; a declared reference does not imply a generated signal. */
export function measureChartSignalCloseExcursions(side:'CALL'|'PUT',entry:number,risk:number,closes:readonly number[]){
 if(!['CALL','PUT'].includes(side)||!positive(entry)||!positive(risk)||!Array.isArray(closes)||closes.length>4096||closes.some(v=>!positive(v)))throw Error('invalid-close-excursion-input');
 const d=side==='CALL'?1:-1,changes=closes.map(v=>d*(v-entry)),mfe=changes.length?Math.max(0,...changes):null,mae=changes.length?Math.max(0,...changes.map(v=>-v)):null;
 const mfeR=mfe===null?null:mfe/risk,maeR=mae===null?null:mae/risk;
 if([mfe,mae,mfeR,maeR].some(v=>v!==null&&!Number.isFinite(v)))throw Error('nonfinite-close-excursion');
 return Object.freeze({samples:closes.length,mfe,mae,mfeR,maeR});
}

/** Incremental independent labeler. First received values are immutable; later replacements are retained as revisions. */
export class ChartSignalOutcomeTracker {
 private readonly ref:OutcomeReference;private readonly ticket:Readonly<ChartSignalTicket>;private readonly key:string;
 private samples=new Map<number,OutcomeSample>();private conflicts=new Set<number>();private revisionKeys=new Set<string>();
 private conflictExamples:{observedAtMs:number;original:OutcomeSample;replacement:OutcomeSample}[]=[];private examples:{observedAtMs:number;original:OutcomeSample;replacement:OutcomeSample}[]=[];private revisionCount=0;private truncated=false;
 private lastAt:number;private lastPublished:number;private rejected=0;private issues=new Set<string>();private endReason:string|null=null;private first:OutcomeRow['firstObservedOutcome']=null;
 constructor(ticket:Readonly<ChartSignalTicket>){this.ref=reference(ticket);this.ticket=freeze(structuredClone(ticket));this.key=chartSignalOutcomeTicketKey(ticket);if(this.key.length>8192)throw Error('outcome-ticket-key-capacity');this.lastAt=this.ref.knownAtMs;this.lastPublished=ticket.publishedMs;}
 get id(){return this.ticket.identity.id;}
 get symbol(){return this.ticket.identity.symbol;}
 get sessionKey(){return this.ticket.identity.sessionKey;}
 get ticketKey(){return this.key;}
 censor(reason:'session-epoch-ended'|'replay-ended'|'ticket-revised'|'source-evidence-revised'){this.endReason=reason;this.issues.add(reason);return this.snapshot();}
 observe(frame:OutcomeFrame):Readonly<OutcomeRow>{
  if(!frame||frame.symbol!==this.symbol||frame.sessionKey!==this.sessionKey||![frame.evaluatedMs,frame.publishedMs].every(clock)||frame.publishedMs>frame.evaluatedMs||frame.evaluatedMs<this.lastAt||frame.publishedMs<this.lastPublished||!Array.isArray(frame.bars)||frame.bars.length>CHART_SIGNAL_OUTCOME_POLICY.maxBarsPerFrame){this.rejected++;this.issues.add('rejected-noncausal-or-foreign-frame');return this.snapshot();}
  this.lastAt=frame.evaluatedMs;this.lastPublished=frame.publishedMs;
  const seen=new Map<number,OutcomeSample>();
  for(const b of frame.bars){
   // Future and pre-confirmation bars are not outcome observations.
   if(!b||!clock(b.timestampMs)){this.issues.add('invalid-bar-excluded');continue;}
   const close=b.timestampMs+minute;if(close<this.ref.firstEligibleCloseMs||close>Math.min(frame.publishedMs,this.ref.deadlineMs)||this.first&&close>this.first.closeMs)continue;
   const v=sample(b,frame,this.ref);if(!v){this.issues.add('invalid-bar-excluded');continue;}
   const same=seen.get(v.startMs);
   if(same&&barKey(same)!==barKey(v)){if(!this.conflicts.has(v.startMs)&&this.conflictExamples.length<CHART_SIGNAL_OUTCOME_POLICY.maxRevisionExamples)this.conflictExamples.push({observedAtMs:frame.evaluatedMs,original:same,replacement:v});this.conflicts.add(v.startMs);this.issues.add('conflicting-same-frame-close');}
   else seen.set(v.startMs,v);
  }
  for(const [start,v]of seen){
   const old=this.samples.get(start);
   if(old&&barKey(old)!==barKey(v)){
    const key=barKey(v);if(!this.revisionKeys.has(key)){
     if(this.revisionKeys.size<CHART_SIGNAL_OUTCOME_POLICY.maxRevisionKeys){this.revisionKeys.add(key);this.revisionCount++;if(this.examples.length<CHART_SIGNAL_OUTCOME_POLICY.maxRevisionExamples)this.examples.push({observedAtMs:frame.evaluatedMs,original:old,replacement:v});else this.truncated=true;}else this.truncated=true;
    }
    this.issues.add('first-observed-bar-revised');
   }else if(!old&&!this.conflicts.has(start)&&this.samples.size<CHART_SIGNAL_OUTCOME_POLICY.maxSamples)this.samples.set(start,v);
  }
  const row=this.snapshot();if(!this.first&&row.firstObservedOutcome)this.first=row.firstObservedOutcome;
  return this.snapshot();
 }
 snapshot():Readonly<OutcomeRow>{
  const r=this.ref,d=r.side==='CALL'?1:-1,lastAllowed=r.sessionOpenMs+Math.floor((r.deadlineMs-r.sessionOpenMs)/minute)*minute;
  const expected=Math.max(0,Math.floor((Math.min(this.lastPublished,r.deadlineMs)-r.firstEligibleCloseMs)/minute)+1),prefix:OutcomeSample[]=[];let missing:number|null=null,out=this.first;
  for(let close=r.firstEligibleCloseMs;close<=lastAllowed;close+=minute){
   const b=this.samples.get(close-minute);if(!b||this.conflicts.has(close-minute)){if(close<=this.lastPublished)missing=close;break;}
   prefix.push(b);
   const kind=d*(b.close-r.target)>=0?'target':d*(b.close-r.invalidation)<0?'invalidation':null;
   if(kind){if(!out)out={kind,closeMs:close,availableMs:Math.max(...prefix.map(v=>v.observedAtMs))};break;}
   if(close>this.lastPublished)break;
  }
  const completeWindow=lastAllowed<r.firstEligibleCloseMs||prefix.at(-1)?.closeMs===lastAllowed;
  if(!out&&this.lastAt>r.deadlineMs&&completeWindow)out={kind:'deadline',closeMs:lastAllowed,availableMs:Math.max(this.lastAt,...prefix.map(v=>v.observedAtMs))};
  const issues=[...this.issues];if(missing!==null)issues.push('missing-close-continuity');
  let outcome:Kind=out?.kind??'pending';
  if(this.endReason&&!out||this.endReason==='ticket-revised'||this.endReason==='source-evidence-revised'||this.revisionCount>0||this.truncated)outcome='censored';
  else if(this.conflicts.size)outcome='ambiguous';
  else if(!out&&this.lastAt>r.deadlineMs&&!completeWindow)outcome='censored';
  const complete=terminal(outcome),measured=measureChartSignalCloseExcursions(r.side,r.entry,r.risk,prefix.map(b=>b.close));
  const full=prefix.filter(b=>b.startMs>=r.knownAtMs&&b.high!==null&&b.low!==null),favorable=full.map(b=>d*((d===1?b.high!:b.low!)-r.entry)),adverse=full.map(b=>-d*((d===1?b.low!:b.high!)-r.entry));
  const targetAt=(b:OutcomeSample)=>d*((d===1?b.high!:b.low!)-r.target)>=0,stopAt=(b:OutcomeSample)=>d*((d===1?b.low!:b.high!)-r.invalidation)<0;
  return freeze({id:this.id,symbol:this.symbol,sessionKey:this.sessionKey,ticketKey:this.key,reference:{...r},asOfMs:this.lastAt,outcome,firstObservedOutcome:out,
   coverage:{expectedCloses:expected,observedCloses:this.samples.size,contiguousCloses:prefix.length,firstMissingCloseMs:missing,completeThroughCloseMs:prefix.at(-1)?.closeMs??null,complete},
   excursions:{basis:'post-confirmation-one-minute-closes',unit:'underlying-price-units',...measured,throughCloseMs:prefix.at(-1)?.closeMs??null,complete},
   ranges:{basis:'fully-after-confirmation-ohlc-subset',unit:'underlying-price-units',bars:full.length,excludedPreKnownBars:prefix.filter(b=>b.startMs<r.knownAtMs).length,missingOhlcBars:prefix.filter(b=>b.startMs>=r.knownAtMs&&(b.high===null||b.low===null)).length,mfe:full.length?Math.max(0,...favorable):null,mae:full.length?Math.max(0,...adverse):null,bothLevelsTouchedBars:full.filter(b=>targetAt(b)&&stopAt(b)).length,fillOrderKnown:false},
   latency:{triggerToConfirmationMs:r.knownAtMs-r.triggerCloseMs,publicationToConfirmationMs:r.knownAtMs-r.publishedMs,outcomeObservationDelayMs:out?out.availableMs-out.closeMs:null},
   samples:[...this.samples.values()].sort((a,b)=>a.startMs-b.startMs),revisions:{count:this.revisionCount,truncated:this.truncated,conflictingStarts:[...this.conflicts].sort((a,b)=>a-b),conflictExamples:[...this.conflictExamples],examples:[...this.examples]},rejectedFrames:this.rejected,issues:[...new Set(issues)].sort(),entryAuthorized:false,executionEnabled:false,probability:null,fill:null,pnl:null});
 }
}

export function chartSignalOutcomesSemanticKey(r:Pick<ChartSignalOutcomesReport,'status'|'rows'|'droppedEpisodes'|'reasonCodes'>):string{return JSON.stringify([r.status,r.droppedEpisodes,r.reasonCodes,r.rows.map(v=>[v.id,v.ticketKey,v.outcome,v.firstObservedOutcome,v.coverage,v.excursions,v.ranges,v.revisions,v.rejectedFrames,v.issues,v.samples.map(b=>[b.startMs,b.close,b.open,b.high,b.low])])]);}
/** Attached to the existing route reader; consumes accepted publications, with no feed/timer/DOM. */
export class ChartSignalOutcomeMonitor {
 private episodes=new Map<string,ChartSignalOutcomeTracker>();private dropped=0;private latest?:Readonly<ChartSignalOutcomesReport>;
 observe(common:Readonly<ChartSignalCoordinatorReport>|undefined,bars:readonly OutcomeBar[],symbol:string,at:number,publishedMs:number|null):Readonly<ChartSignalOutcomesReport>{
  const associated=!!common&&validChartSignalCoordinatorReport(common)&&common.status==='observed'&&common.symbol===symbol&&common.evaluatedMs===at&&common.publishedMs===publishedMs;
  const reasons:string[]=[];if(!associated)reasons.push('coordinator-not-associated');
  if(associated){
   for(const tracker of this.episodes.values())if(tracker.sessionKey!==common.sessionKey)tracker.censor('session-epoch-ended');
   for(const row of common.signals){const ticket=row.ticket;if(!ticket)continue;let t=this.episodes.get(ticket.identity.id);
    if(!t&&ticket.confirmedAtMs===at){t=new ChartSignalOutcomeTracker(ticket);this.episodes.set(t.id,t);}
    else if(!t)reasons.push('older-ticket-not-backfilled');
    if(t&&t.ticketKey!==chartSignalOutcomeTicketKey(ticket))t.censor('ticket-revised');
    if(t&&row.revisionObserved)t.censor('source-evidence-revised');
   }
   for(const tracker of this.episodes.values())if(tracker.symbol===symbol&&tracker.sessionKey===common.sessionKey)tracker.observe({symbol,sessionKey:common.sessionKey!,evaluatedMs:at,publishedMs:publishedMs!,bars});
  }
  while(this.episodes.size>CHART_SIGNAL_OUTCOME_POLICY.maxEpisodes){this.episodes.delete(this.episodes.keys().next().value!);this.dropped++;}
  const make=():ChartSignalOutcomesReport=>{const report:ChartSignalOutcomesReport={version:CHART_SIGNAL_OUTCOME_POLICY.version,mode:'shadow',symbol,sessionKey:common?.sessionKey??null,evaluatedMs:clock(at)?at:null,publishedMs:clock(publishedMs)?publishedMs:null,status:associated?'observed':'waiting',scope:'current-and-recent-observed-episodes',rows:[...this.episodes.values()].map(v=>v.snapshot()),droppedEpisodes:this.dropped,reasonCodes:[...new Set(reasons)].sort(),semanticKey:'',entryAuthorized:false,executionEnabled:false,markersEnabled:false,probability:null,fill:null,pnl:null};report.semanticKey=chartSignalOutcomesSemanticKey(report);return report;};
  let report=make();while(encoder.encode(JSON.stringify(report)).byteLength>CHART_SIGNAL_OUTCOME_POLICY.maxReportBytes&&this.episodes.size){this.episodes.delete(this.episodes.keys().next().value!);this.dropped++;reasons.push('outcome-byte-capacity');report=make();}
  this.latest=freeze(report);return this.latest;
 }
 snapshot(symbol:string,now:number){const r=this.latest;return clock(now)&&r?.symbol===symbol&&r.evaluatedMs!==null&&r.evaluatedMs<=now&&now-r.evaluatedMs<=3000?r:undefined;}
}

/** Causal ordered replay: never sorts frames, rewrites tickets, or infers missing future closes. */
export function replayChartSignalOutcome(ticket:Readonly<ChartSignalTicket>,frames:readonly OutcomeFrame[],manifest:{sourceId:string;asOfMs:number;ended:boolean}){
 if(!text(manifest?.sourceId)||!clock(manifest?.asOfMs)||typeof manifest.ended!=='boolean'||!Array.isArray(frames)||frames.length>4096)throw Error('invalid-outcome-replay-manifest');
 const tracker=new ChartSignalOutcomeTracker(ticket);if(manifest.asOfMs<ticket.confirmedAtMs)throw Error('replay-asof-before-ticket');
 for(const frame of frames){if(frame.evaluatedMs>manifest.asOfMs)throw Error('replay-frame-after-asof');tracker.observe(frame);}
 if(manifest.ended)tracker.censor('replay-ended');return freeze({version:CHART_SIGNAL_OUTCOME_POLICY.version,sourceId:manifest.sourceId,asOfMs:manifest.asOfMs,inputFrames:frames.length,ended:manifest.ended,outcome:tracker.snapshot(),sourceTerminalStateUsed:false,fill:null,pnl:null,probability:null,executionEnabled:false});
}

export function validChartSignalOutcomesReport(value:unknown):value is Readonly<ChartSignalOutcomesReport>{
 try{const r=value as ChartSignalOutcomesReport;
  if(!r||r.version!==CHART_SIGNAL_OUTCOME_POLICY.version||r.mode!=='shadow'||!text(r.symbol)||!['observed','waiting'].includes(r.status)||r.scope!=='current-and-recent-observed-episodes'||!(r.sessionKey===null||text(r.sessionKey))||![r.evaluatedMs,r.publishedMs].every(v=>v===null||clock(v))||r.status==='observed'&&(!clock(r.evaluatedMs)||!clock(r.publishedMs)||r.publishedMs>r.evaluatedMs||!text(r.sessionKey))||!Array.isArray(r.rows)||r.rows.length>4||!Number.isSafeInteger(r.droppedEpisodes)||r.droppedEpisodes<0||r.entryAuthorized!==false||r.executionEnabled!==false||r.markersEnabled!==false||r.fill!==null||r.pnl!==null||r.probability!==null||!Array.isArray(r.reasonCodes)||r.reasonCodes.some(v=>!text(v))||encoder.encode(JSON.stringify(r)).byteLength>CHART_SIGNAL_OUTCOME_POLICY.maxReportBytes||r.semanticKey!==chartSignalOutcomesSemanticKey(r))return false;
  const ids=new Set<string>();for(const v of r.rows as readonly Readonly<OutcomeRow>[]){
   const p=v.reference,d=p?.side==='CALL'?1:p?.side==='PUT'?-1:0;
   if(!v||!text(v.id)||ids.has(v.id)||!text(v.symbol)||!text(v.sessionKey)||typeof v.ticketKey!=='string'||v.ticketKey.length>8192||!clock(v.asOfMs)||v.asOfMs>r.evaluatedMs!||!d||![p.entry,p.target,p.invalidation,p.risk,p.reward,p.rewardRisk].every(positive)||p.risk!==d*(p.entry-p.invalidation)||p.reward!==d*(p.target-p.entry)||p.rewardRisk!==p.reward/p.risk||!['pending','target','invalidation','deadline','censored','ambiguous'].includes(v.outcome)||!Array.isArray(v.samples)||v.samples.length>10||v.entryAuthorized!==false||v.executionEnabled!==false||v.probability!==null||v.fill!==null||v.pnl!==null||v.ranges?.fillOrderKnown!==false||v.excursions?.basis!=='post-confirmation-one-minute-closes'||v.excursions.unit!=='underlying-price-units'||v.coverage.complete!==terminal(v.outcome)||v.excursions.complete!==terminal(v.outcome))return false;
   if(![p.knownAtMs,p.triggerCloseMs,p.publishedMs,p.deadlineMs,p.sessionOpenMs,p.sessionCloseMs,p.firstEligibleCloseMs].every(clock)||p.publishedMs>p.knownAtMs||p.triggerCloseMs>p.knownAtMs||p.knownAtMs>=p.deadlineMs||p.deadlineMs>p.sessionCloseMs||p.sessionOpenMs>=p.triggerCloseMs||p.firstEligibleCloseMs!==p.sessionOpenMs+(Math.floor((p.knownAtMs-p.sessionOpenMs)/minute)+1)*minute||v.asOfMs<p.knownAtMs||!v.revisions||!Number.isSafeInteger(v.revisions.count)||v.revisions.count<0||!Array.isArray(v.revisions.conflictingStarts)||v.revisions.conflictingStarts.some((n:unknown)=>!clock(n))||!Array.isArray(v.issues)||v.issues.some((c:unknown)=>!text(c)))return false;
   ids.add(v.id);let previous=0;for(const b of v.samples){if(!clock(b.startMs)||b.startMs<=previous||b.closeMs!==b.startMs+minute||(b.startMs-p.sessionOpenMs)%minute!==0||!positive(b.close)||!clock(b.publishedMs)||!clock(b.observedAtMs)||b.closeMs>Math.min(b.publishedMs,p.deadlineMs)||b.publishedMs>b.observedAtMs||b.observedAtMs>v.asOfMs||b.observedAtMs<p.knownAtMs||b.closeMs<p.firstEligibleCloseMs||!sample({timestampMs:b.startMs,close:b.close,open:b.open,high:b.high,low:b.low},{symbol:v.symbol,sessionKey:v.sessionKey,publishedMs:b.publishedMs,evaluatedMs:b.observedAtMs,bars:[]},p))return false;previous=b.startMs;}
   if(v.firstObservedOutcome&&(!terminal(v.firstObservedOutcome.kind)||!clock(v.firstObservedOutcome.closeMs)||!clock(v.firstObservedOutcome.availableMs)||v.firstObservedOutcome.availableMs<v.firstObservedOutcome.closeMs||v.firstObservedOutcome.availableMs>v.asOfMs))return false;
   const byStart=new Map<number,OutcomeSample>(v.samples.map((b:OutcomeSample)=>[b.startMs,b])),prefix:OutcomeSample[]=[];for(let close=p.firstEligibleCloseMs;close<=p.deadlineMs;close+=minute){const b=byStart.get(close-minute);if(!b||v.revisions.conflictingStarts.includes(b.startMs))break;prefix.push(b);if(d*(b.close-p.target)>=0||d*(b.close-p.invalidation)<0)break;}
   const measured=measureChartSignalCloseExcursions(p.side,p.entry,p.risk,prefix.map(b=>b.close));
   if(v.coverage.observedCloses!==v.samples.length||v.coverage.contiguousCloses!==prefix.length||v.coverage.completeThroughCloseMs!==(prefix.at(-1)?.closeMs??null)||v.excursions.samples!==measured.samples||v.excursions.throughCloseMs!==(prefix.at(-1)?.closeMs??null)||(['mfe','mae','mfeR','maeR']as const).some(k=>v.excursions[k]!==measured[k])||v.latency.triggerToConfirmationMs!==p.knownAtMs-p.triggerCloseMs||v.latency.publicationToConfirmationMs!==p.knownAtMs-p.publishedMs||v.latency.outcomeObservationDelayMs!==(v.firstObservedOutcome?v.firstObservedOutcome.availableMs-v.firstObservedOutcome.closeMs:null))return false;
   if(terminal(v.outcome)&&v.firstObservedOutcome?.kind!==v.outcome)return false;
   for(const n of [v.excursions.mfe,v.excursions.mae,v.excursions.mfeR,v.excursions.maeR,v.ranges.mfe,v.ranges.mae])if(n!==null&&(typeof n!=='number'||!Number.isFinite(n)||n<0))return false;
  }return true;
 }catch{return false;}
}
