import {CHART_SIGNAL_STUDY_POLICY as P,CHART_SIGNAL_STUDY_REGISTRY_SIGNATURE as registrySignature,type StudyGate} from './chart-signal-study-policy';
import {projectChartSignal,type ChartSignalTicket} from './chart-signal-contract';
import {validChartSignalCoordinatorReport,type ChartSignalCoordinatorReport} from './chart-signal-coordinator';
import {chartSignalOutcomeTicketKey,validChartSignalOutcomesReport,type ChartSignalOutcomesReport,type OutcomeSample} from './chart-signal-outcomes';
export {CHART_SIGNAL_STUDY_POLICY,CHART_SIGNAL_STUDY_REGISTRY_SIGNATURE}from'./chart-signal-study-policy';
export type TrialOrigin='native-accepted-payload'|'snapshot-reference';
export interface StudyContext {
 symbol:string;atMs:number;frame:{sessionKey:string|null;publishedMs:number|null;status:string};
 price:readonly {interval:number;status:string;side:string;closedAtMs:number|null;reference:boolean;lastClose:number|null;atr:number|null}[];
 technical:readonly {interval:number;status:string;closedAtMs:number|null;trend:string|null;adx:number|null;rvol:number|null;rvolReferences:number|null}[];
 activity:{flow:{status:string;observedMs:number|null;signal:number|null;classifiedCoveragePct:number|null;temporalCoverage:string;intervalMinutes:number;unit:string}};
 options:{status:string;sourceStatus:string|null;observedMs:number|null;id:string|null;priceDirectionInferred:false};
 patterns:{status:string;observedMs:number|null;fvg:string|null;squeeze:string|null;imbalance:string|null};absorption:{status:string;observedMs:number|null};
}
export interface StudyGateAssessment {gate:StudyGate;decision:'accept'|'reject'|'unavailable';observedMs:number|null;unit:string;values:readonly (string|number|null)[];reason:string}
export interface StudyEnrollment {version:'chart-signal-enrollment-v1';id:string;registryId:string;origin:TrialOrigin;sourceCaseId:string;knownAtMs:number;ticket:Readonly<ChartSignalTicket>;ticketKey:string;assessments:readonly StudyGateAssessment[]}
export interface ChartSignalTrialFrame {version:'chart-signal-trials-v1';registryId:string;registrySignature:string;symbol:string;sessionKey:string|null;evaluatedMs:number|null;publishedMs:number|null;origin:TrialOrigin;status:'observed'|'waiting';enrollments:readonly Readonly<StudyEnrollment>[];droppedEnrollments:number;reasonCodes:readonly string[];semanticKey:string;entryAuthorized:false;executionEnabled:false;markersEnabled:false;probability:null;fill:null;pnl:null}
export interface StudyCase {sequence:number;caseId:string;symbol:string;sessionKey:string|null;publishedMs:number|null;evaluatedMs:number;receivedMs:number;captureMissed:number;trialStatus?:string;trials?:Readonly<ChartSignalTrialFrame>|null;outcomeStatus?:string;outcomes?:Readonly<ChartSignalOutcomesReport>|null}
export interface StudyManifest {sourceId:string;asOfMs:number;complete:boolean;conflicts:number;sourceMode:'native-persisted-derived-cases'|'snapshot-reference';omittedRows?:number}
const clock=(v:unknown):v is number=>typeof v==='number'&&Number.isSafeInteger(v)&&v>0&&v<=8640000000000000;
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v);
const positive=(v:unknown):v is number=>finite(v)&&v>0;
const text=(v:unknown):v is string=>typeof v==='string'&&v.length>0&&v.length<=8192;
const day=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'}),bytes=(v:unknown)=>new TextEncoder().encode(JSON.stringify(v)).byteLength;
function freeze<T>(v:T):Readonly<T>{if(v&&typeof v==='object'&&!Object.isFrozen(v)){Object.values(v).forEach(freeze);Object.freeze(v);}return v;}
const key=(v:unknown):string=>Array.isArray(v)?'['+v.map(key).join(',')+']':v&&typeof v==='object'?'{'+Object.entries(v).sort(([a],[b])=>a<b?-1:a>b?1:0).map(([k,x])=>JSON.stringify(k)+':'+key(x)).join(',')+'}':JSON.stringify(v)??'undefined';
function assessments(t:{side:ChartSignalTicket['side'];identity:Pick<ChartSignalTicket['identity'],'symbol'|'sessionKey'>;triggerCloseMs:number},c:StudyContext|null|undefined,at:number,published:number):StudyGateAssessment[]{
 const associated=!!c&&c.symbol===t.identity.symbol&&c.atMs===at&&c.frame.sessionKey===t.identity.sessionKey&&c.frame.publishedMs===published&&c.frame.status==='connected';
 const own=(gate:StudyGate,known:boolean,accept:boolean,observedMs:number|null,unit:string,values:(string|number|null)[],reason:string):StudyGateAssessment=>({gate,decision:known?accept?'accept':'reject':'unavailable',observedMs:known?observedMs:null,unit,values:known?values:[],reason:known?reason:'source-not-associated-or-available'});
 const closed=(v:{status:string;closedAtMs:number|null}|undefined)=>associated&&v?.status==='connected'&&clock(v.closedAtMs)&&v.closedAtMs<=published;
 const p15=c?.price.find(v=>v.interval===15),p60=c?.price.find(v=>v.interval===60),tech=c?.technical.find(v=>v.interval===15),flow=c?.activity.flow,gex=c?.options,patterns=c?.patterns;
 const price=(gate:StudyGate,v:typeof p15)=>own(gate,!!v&&closed(v)&&!v.reference&&['CALL','PUT','MIXTA'].includes(v.side),v?.side===t.side,v?.closedAtMs??null,'closed-underlying-price',[v?.side??null,v?.lastClose??null,v?.atr??null],'same-closed-structure-side');
 const techKnown=!!tech&&closed(tech)&&closed(p15)&&tech.closedAtMs===p15?.closedAtMs&&finite(tech.adx)&&finite(tech.rvol)&&tech.rvolReferences!==null&&tech.rvolReferences>=P.technicalRequiredReferences;
 const flowKnown=associated&&flow?.status==='connected'&&flow.temporalCoverage==='complete'&&flow.intervalMinutes===5&&flow.unit==='normalized-premium'&&clock(flow.observedMs)&&flow.observedMs<=published&&published-flow.observedMs<=3000&&finite(flow.signal)&&Math.abs(flow.signal)<=1&&finite(flow.classifiedCoveragePct)&&flow.classifiedCoveragePct>=P.optionClassifiedMinimumPct&&flow.classifiedCoveragePct<=100;
 const gexKnown=associated&&gex?.status==='connected'&&gex.sourceStatus==='available'&&clock(gex.observedMs)&&gex.observedMs<=published&&text(gex.id)&&gex.priceDirectionInferred===false;
 const patternKnown=associated&&patterns?.status==='connected'&&clock(patterns.observedMs)&&patterns.observedMs<=published;
 return [{gate:'baseline',decision:'accept',observedMs:t.triggerCloseMs,unit:'original-qualified-source-event',values:[t.side],reason:'existing-source-confirmation-only'},price('structure-15m-aligned',p15),price('structure-60m-aligned',p60),
  own('technical-15m-aligned',techKnown,tech?.trend===t.side&&tech.adx!>=P.technicalAdxMinimum&&tech.rvol!>=P.technicalRvolMinimum,tech?.closedAtMs??null,'ADX-and-relative-volume',[tech?.trend??null,tech?.adx??null,tech?.rvol??null,tech?.rvolReferences??null],'registered-trend-adx20-rvol1-references20'),
  own('options-premium-5m-aligned',!!flowKnown,(t.side==='CALL'?1:-1)*(flow?.signal??0)>0,flow?.observedMs??null,'normalized-premium-5m',[flow?.signal??null,flow?.classifiedCoveragePct??null],'existing-complete-classified-premium;not-independent-of-source-options'),
  own('gex-context-covered',!!gexKnown,true,gex?.observedMs??null,'availability-only',[gex?.id??null],'covered-options-context;no-direction-inferred'),
  own('patterns-15m-covered',!!patternKnown,true,patterns?.observedMs??null,'availability-only',[patterns?.fvg??null,patterns?.squeeze??null,patterns?.imbalance??null],'same-15m-description-family;not-three-independent-votes'),
  {gate:'absorption-direction-adapter',decision:'unavailable',observedMs:null,unit:'not-associated',values:[],reason:'existing-connection-has-no-directional-absorption-adapter'}];
}
/** Declared-direction measurements only. This function cannot create a qualified source signal. */
export function measureStudyFamilyGates(input:{symbol:string;sessionKey:string;side:'CALL'|'PUT';triggerCloseMs:number;evaluatedMs:number;publishedMs:number},context:StudyContext|null){
 const measured=assessments({side:input.side,identity:{symbol:input.symbol,sessionKey:input.sessionKey},triggerCloseMs:input.triggerCloseMs},context,input.evaluatedMs,input.publishedMs).filter(v=>v.gate!=='baseline');
 return freeze({measurements:measured,sourceSignalQualified:false as const,entryAuthorized:false as const,executionEnabled:false as const,probability:null});
}
export function studyReferenceCostR(entry:number,risk:number,roundTripBps:number){if(!positive(entry)||!positive(risk)||!finite(roundTripBps)||roundTripBps<0||roundTripBps>10000)throw Error('invalid-reference-cost');const value=roundTripBps/10000*entry/risk;if(!finite(value))throw Error('nonfinite-reference-cost');return value;}

export function chartSignalTrialSemanticKey(v:Pick<ChartSignalTrialFrame,'origin'|'status'|'enrollments'|'droppedEnrollments'|'reasonCodes'>){return JSON.stringify([P.registryId,v.origin,v.status,v.droppedEnrollments,v.reasonCodes,v.enrollments.map(e=>[e.id,e.sourceCaseId,e.ticketKey,e.assessments])]);}
/** Freezes features only at original confirmation, before any outcome. Four recent envelopes, no timer/feed/writer. */
export class ChartSignalTrialRecorder {
 private symbol='';private recent=new Map<string,Readonly<StudyEnrollment>>();private dropped=0;
 observe(input:{common:Readonly<ChartSignalCoordinatorReport>|null|undefined;context:StudyContext|null|undefined;symbol:string;at:number;publishedMs:number|null;caseId:string;origin:TrialOrigin}):Readonly<ChartSignalTrialFrame>{
  const {common,symbol,at,publishedMs,origin,caseId}=input,reasons:string[]=[],introduced:string[]=[];
  if(symbol!==this.symbol){this.recent.clear();this.symbol=symbol;this.dropped=0;}
  const associated=!!common&&validChartSignalCoordinatorReport(common)&&common.status==='observed'&&common.symbol===symbol&&common.evaluatedMs===at&&common.publishedMs===publishedMs;
  if(!associated)reasons.push('coordinator-not-associated');
  if(associated)for(const row of common.signals){const t=row.ticket;if(!t||this.recent.has(t.identity.id))continue;if(t.confirmedAtMs!==at){reasons.push('old-ticket-not-enrolled');continue;}
   const e:StudyEnrollment={version:'chart-signal-enrollment-v1',id:t.identity.id,registryId:P.registryId,origin,sourceCaseId:caseId,knownAtMs:at,ticket:structuredClone(t),ticketKey:chartSignalOutcomeTicketKey(t),assessments:assessments(t,input.context,at,publishedMs!)};this.recent.set(e.id,freeze(e));introduced.push(e.id);
  }
  const make=():ChartSignalTrialFrame=>{const r:ChartSignalTrialFrame={version:'chart-signal-trials-v1',registryId:P.registryId,registrySignature,symbol,sessionKey:common?.sessionKey??null,evaluatedMs:clock(at)?at:null,publishedMs:clock(publishedMs)?publishedMs:null,origin,status:associated?'observed':'waiting',enrollments:[...this.recent.values()],droppedEnrollments:this.dropped,reasonCodes:[...new Set([...reasons,...(introduced.some(id=>!this.recent.has(id))?['new-enrollment-not-retained']:[])])].sort(),semanticKey:'',entryAuthorized:false,executionEnabled:false,markersEnabled:false,probability:null,fill:null,pnl:null};r.semanticKey=chartSignalTrialSemanticKey(r);return r;};
  while(this.recent.size>P.maxRecentEnrollments){this.recent.delete(this.recent.keys().next().value!);this.dropped++;}
  let r=make();while(bytes(r)>P.maxTrialBytes&&this.recent.size){this.recent.delete(this.recent.keys().next().value!);this.dropped++;reasons.push('trial-envelope-byte-capacity');r=make();}return freeze(r);
 }
}
export function validChartSignalTrialFrame(value:unknown):value is Readonly<ChartSignalTrialFrame>{
 try{const r=value as ChartSignalTrialFrame;if(!r||r.version!=='chart-signal-trials-v1'||r.registryId!==P.registryId||r.registrySignature!==registrySignature||!text(r.symbol)||!['native-accepted-payload','snapshot-reference'].includes(r.origin)||!['observed','waiting'].includes(r.status)||![r.evaluatedMs,r.publishedMs].every(v=>v===null||clock(v))||r.evaluatedMs!==null&&r.publishedMs!==null&&r.publishedMs>r.evaluatedMs||!Array.isArray(r.enrollments)||r.enrollments.length>4||!Number.isSafeInteger(r.droppedEnrollments)||r.droppedEnrollments<0||bytes(r)>P.maxTrialBytes||r.semanticKey!==chartSignalTrialSemanticKey(r)||r.entryAuthorized!==false||r.executionEnabled!==false||r.markersEnabled!==false||r.probability!==null||r.fill!==null||r.pnl!==null)return false;
  const ids=new Set<string>();for(const e of r.enrollments as readonly Readonly<StudyEnrollment>[]){
   if(!e||e.version!=='chart-signal-enrollment-v1'||e.registryId!==P.registryId||e.origin!==r.origin||!text(e.id)||ids.has(e.id)||!text(e.sourceCaseId)||e.id!==e.ticket.identity.id||e.ticket.identity.symbol!==r.symbol||!clock(e.knownAtMs)||e.knownAtMs!==e.ticket.confirmedAtMs||e.knownAtMs>r.evaluatedMs!||e.ticketKey!==chartSignalOutcomeTicketKey(e.ticket)||!projectChartSignal(null,{eventId:'validate-study-ticket',kind:'confirm',evaluatedMs:e.knownAtMs,proposal:e.ticket}).accepted||!Array.isArray(e.assessments)||e.assessments.length!==P.gates.length||new Set(e.assessments.map(v=>v.gate)).size!==P.gates.length)return false;ids.add(e.id);
   for(const a of e.assessments){
    if(!P.gates.includes(a.gate)||!['accept','reject','unavailable'].includes(a.decision)||!text(a.unit)||!text(a.reason)||!Array.isArray(a.values)||a.values.length>8||a.values.some((v:unknown)=>!(v===null||typeof v==='string'&&v.length<=2048||finite(v)))||(a.decision==='unavailable'?(a.observedMs!==null||a.values.length!==0):(!clock(a.observedMs)||a.observedMs>e.ticket.publishedMs)))return false;
    if(a.decision==='unavailable')continue;
    let expected:boolean;const v=a.values;
    if(a.gate==='baseline'){if(v.length!==1||v[0]!==e.ticket.side)return false;expected=true;}
    else if(a.gate==='structure-15m-aligned'||a.gate==='structure-60m-aligned'){if(v.length!==3||!['CALL','PUT','MIXTA'].includes(String(v[0]))||!positive(v[1])||!positive(v[2]))return false;expected=v[0]===e.ticket.side;}
    else if(a.gate==='technical-15m-aligned'){if(v.length!==4||typeof v[0]!=='string'||!finite(v[1])||v[1]<0||!finite(v[2])||v[2]<0||!finite(v[3])||v[3]<20)return false;expected=v[0]===e.ticket.side&&v[1]>=20&&v[2]>=1;}
    else if(a.gate==='options-premium-5m-aligned'){if(v.length!==2||!finite(v[0])||Math.abs(v[0])>1||!finite(v[1])||v[1]<70||v[1]>100)return false;expected=(e.ticket.side==='CALL'?1:-1)*v[0]>0;}
    else if(a.gate==='gex-context-covered'){if(v.length!==1||!text(v[0]))return false;expected=true;}
    else if(a.gate==='patterns-15m-covered'){if(v.length!==3)return false;expected=true;}
    else return false;
    if(a.decision!==(expected?'accept':'reject'))return false;
   }
  }return true;
 }catch{return false;}
}
export type StudyPartition='warmup'|'training'|'validation'|'holdout'|'after-registered-window';
export function chartSignalStudyPartition(date:string):StudyPartition{return date<P.trainFrom?'warmup':date<P.validationFrom?'training':date<P.holdoutFrom?'validation':date<P.holdoutUntil?'holdout':'after-registered-window';}
/** Wilson reference interval assumes IID Bernoulli observations. That assumption is NOT certified for market events. */
export function studyWilsonReference(successes:number,n:number){if(!Number.isSafeInteger(n)||!Number.isSafeInteger(successes)||n<0||successes<0||successes>n)throw Error('invalid-wilson-counts');if(!n)return null;const p=successes/n,z=P.wilsonZ,z2=z*z,denom=1+z2/n,center=(p+z2/(2*n))/denom,half=z*Math.sqrt(p*(1-p)/n+z2/(4*n*n))/denom;return {low:successes===0?0:Math.max(0,center-half),high:successes===n?1:Math.min(1,center+half),confidence:.95,assumption:'hypothetical-iid-not-established-for-market-events' as const};}
interface Episode {enrollment:Readonly<StudyEnrollment>;dateET:string;partition:StudyPartition;primitive:string;samples:Map<number,OutcomeSample>;issues:Set<string>;endObserved:boolean;label:'pending'|'target'|'invalidation'|'deadline'|'censored'|'ambiguous';grossCloseReferenceR:number|null;labelAvailableMs:number|null;terminalCloseMs:number|null;purged:string|null}
function outcome(e:Episode,asOf:number){
 const t=e.enrollment.ticket,d=t.side==='CALL'?1:-1,known=Math.max(t.availableMs,t.confirmedAtMs),first=t.session.openMs+(Math.floor((known-t.session.openMs)/60000)+1)*60000;let last:OutcomeSample|null=null,available=known;
 e.label='pending';e.grossCloseReferenceR=null;e.labelAvailableMs=null;e.terminalCloseMs=null;
 for(let end=first;end<=t.deadlineMs;end+=60000){const b=e.samples.get(end-60000);if(!b)break;last=b;available=Math.max(available,b.observedAtMs);if(d*(b.close-t.target!.price)>=0||d*(b.close-t.invalidation!.price)<0){e.label=d*(b.close-t.target!.price)>=0?'target':'invalidation';break;}}
 if(e.label==='pending'&&asOf>t.deadlineMs&&last?.closeMs===t.deadlineMs){e.label='deadline';available=Math.max(available,t.deadlineMs+1);}
 if(e.issues.size||e.label==='pending'&&(asOf>t.deadlineMs||e.endObserved))e.label=e.issues.has('conflicting-same-frame-close')?'ambiguous':'censored';
 if(['target','invalidation','deadline'].includes(e.label)&&last){e.grossCloseReferenceR=d*(last.close-t.entryReference!.price)/t.geometry.risk;e.labelAvailableMs=available;e.terminalCloseMs=last.closeMs;}
}
const mean=(v:number[])=>v.length?v.reduce((a,b)=>a+b/v.length,0):null;
export function compareChartSignalStudy(rows:readonly StudyCase[],manifest:StudyManifest){
 const issues=new Set<string>(),episodes=new Map<string,Episode>(),seenCases=new Map<string,string>(),scopeDates=new Map<string,number>();let lastSequence=0,duplicates=0,referenceRows=0,byteCount=0,invalidRows=0,unknownProtocolRows=0,recentTrialEvictions=0,recentOutcomeEvictions=0;
 const validManifest=!!manifest&&text(manifest.sourceId)&&clock(manifest.asOfMs)&&typeof manifest.complete==='boolean'&&Number.isSafeInteger(manifest.conflicts)&&manifest.conflicts>=0&&['native-persisted-derived-cases','snapshot-reference'].includes(manifest.sourceMode);
 if(!validManifest)issues.add('invalid-study-manifest');if(!manifest?.complete)issues.add('incomplete-compact-source-window');if(manifest?.conflicts)issues.add('original-source-id-conflicts');if(!Array.isArray(rows)||rows.length>P.maxStudyRows)issues.add('study-row-capacity');
 for(const r of Array.isArray(rows)?rows.slice(0,P.maxStudyRows):[]){
  let encoded:string;try{encoded=key(r);byteCount+=bytes(r);}catch{invalidRows++;issues.add('non-json-study-case');continue;}if(byteCount>P.maxInputBytes){issues.add('study-byte-capacity');break;}
  const old=seenCases.get(r.caseId);if(old!==undefined){if(old===encoded)duplicates++;else issues.add('study-case-id-conflict');continue;}seenCases.set(r.caseId,encoded);
  if(!r||!text(r.caseId)||!text(r.symbol)||!Number.isSafeInteger(r.sequence)||r.sequence<=lastSequence||![r.evaluatedMs,r.receivedMs,r.publishedMs].every(clock)||r.publishedMs!>r.evaluatedMs||r.evaluatedMs>r.receivedMs||r.receivedMs>manifest.asOfMs||!Number.isSafeInteger(r.captureMissed)||r.captureMissed<0){invalidRows++;issues.add('invalid-case-clock-or-order');continue;}lastSequence=r.sequence;
  const date=day.format(r.evaluatedMs),f=r.trials;
  if(!f){unknownProtocolRows++;if(date>=P.trainFrom)issues.add('missing-trial-protocol-in-registered-session');continue;}
  if(!validChartSignalTrialFrame(f)||f.symbol!==r.symbol||f.sessionKey!==r.sessionKey&&!(f.status==='waiting'&&f.sessionKey===null&&f.enrollments.length===0)||f.evaluatedMs!==r.evaluatedMs||f.publishedMs!==r.publishedMs||r.trialStatus!=='associated'){invalidRows++;issues.add('trial-protocol-or-association-invalid');continue;}
  if(f.origin!=='native-accepted-payload'||manifest.sourceMode!=='native-persisted-derived-cases'){referenceRows++;continue;}
  recentTrialEvictions=Math.max(recentTrialEvictions,f.droppedEnrollments);if(f.reasonCodes.includes('new-enrollment-not-retained'))issues.add('new-enrollment-not-retained');if(r.captureMissed)issues.add('known-source-capture-gap');
  const o=r.outcomes,associated=!!o&&r.outcomeStatus==='associated'&&validChartSignalOutcomesReport(o)&&o.symbol===r.symbol&&o.sessionKey===r.sessionKey&&o.evaluatedMs===r.evaluatedMs&&o.publishedMs===r.publishedMs;
  if(o&& !associated)issues.add('outcome-protocol-or-association-invalid');if(associated)recentOutcomeEvictions=Math.max(recentOutcomeEvictions,o.droppedEpisodes);
  for(const n of f.enrollments){
   if(n.origin!=='native-accepted-payload'||n.knownAtMs<P.registeredAtMs){referenceRows++;continue;}
   const existing=episodes.get(n.id);
   if(existing){if(key(existing.enrollment)!==key(n))existing.issues.add('frozen-enrollment-revised');continue;}
   if(n.sourceCaseId!==r.caseId||n.knownAtMs!==r.evaluatedMs){issues.add('initial-enrollment-not-observed');continue;}
   if(n.origin!=='native-accepted-payload'||n.knownAtMs<P.registeredAtMs){referenceRows++;continue;}
   if(episodes.size>=P.maxEpisodes){issues.add('study-episode-capacity');continue;}
   const t=n.ticket,dateET=day.format(n.knownAtMs),primitive=key([t.identity.symbol,dateET,t.side,t.impulseCloseMs,t.triggerCloseMs,t.entryReference!.price,t.target!.price,t.invalidation!.price]);
   episodes.set(n.id,{enrollment:n,dateET,partition:chartSignalStudyPartition(dateET),primitive,samples:new Map(),issues:new Set(),endObserved:false,label:'pending',grossCloseReferenceR:null,labelAvailableMs:null,terminalCloseMs:null,purged:null});scopeDates.set(dateET,Math.max(scopeDates.get(dateET)??0,t.session.closeMs));
  }
  if(associated)for(const row of o.rows){const e=episodes.get(row.id);if(!e)continue;if(row.ticketKey!==e.enrollment.ticketKey){e.issues.add('ticket-key-revised');continue;}
   if(row.revisions.count||row.revisions.truncated)e.issues.add('first-observed-bar-revised');if(row.revisions.conflictingStarts.length)e.issues.add('conflicting-same-frame-close');
   for(const code of row.issues)if(['source-evidence-revised','ticket-revised'].includes(code))e.issues.add(code);e.endObserved ||=row.issues.includes('session-epoch-ended');
   for(const b of row.samples){const prior=e.samples.get(b.startMs);if(prior&&key(prior)!==key(b))e.issues.add('persisted-sample-revised');else if(!prior)e.samples.set(b.startMs,b);}
  }
 }
 if(invalidRows)issues.add('invalid-study-rows');
 const list=[...episodes.values()].sort((a,b)=>a.enrollment.knownAtMs-b.enrollment.knownAtMs||a.enrollment.id.localeCompare(b.enrollment.id)),primitive=new Set<string>(),horizons=new Map<string,number>();
 for(const e of list){outcome(e,manifest.asOfMs);const t=e.enrollment.ticket;
  if(primitive.has(e.primitive)){e.purged='duplicate-primitive-across-epochs';continue;}primitive.add(e.primitive);
  const boundary=e.partition==='training'?P.validationFrom:e.partition==='validation'?P.holdoutFrom:null;
  if(boundary&&day.format(t.deadlineMs)>=boundary){e.purged='cross-partition-horizon';continue;}
  if(e.enrollment.knownAtMs<=(horizons.get(t.identity.symbol)??0)){e.purged='overlapping-frozen-horizon';continue;}horizons.set(t.identity.symbol,t.deadlineMs);
 }
 const completedDates=Object.fromEntries((['training','validation','holdout']as const).map(p=>[p,[...scopeDates].filter(([date,end])=>chartSignalStudyPartition(date)===p&&end<=manifest.asOfMs).map(([date])=>date).sort()]));
 const sourceSafe=validManifest&&manifest.complete&&manifest.conflicts===0&&issues.size===0&&manifest.sourceMode==='native-persisted-derived-cases';
 const groups=(['warmup','training','validation','holdout','after-registered-window']as const).map(partition=>{
  const population=list.filter(e=>e.partition===partition&&!e.purged),summaries=P.gates.map(gate=>{
   const selected=population.filter(e=>e.enrollment.assessments.find(a=>a.gate===gate)?.decision==='accept'),known=population.filter(e=>e.enrollment.assessments.find(a=>a.gate===gate)?.decision!=='unavailable'),closed=selected.filter(e=>e.grossCloseReferenceR!==null),paired=known.filter(e=>e.grossCloseReferenceR!==null),targets=closed.filter(e=>e.label==='target').length;
   const counts={pending:0,target:0,invalidation:0,deadline:0,censored:0,ambiguous:0};selected.forEach(e=>counts[e.label]++);
   const costs=P.costScenariosRoundTripBps.map(roundTripBps=>{const adjusted=(e:Episode)=>e.grossCloseReferenceR!-studyReferenceCostR(e.enrollment.ticket.entryReference!.price,e.enrollment.ticket.geometry.risk,roundTripBps);return {roundTripBps,scenarioClass:'declared-underlying-reference-sensitivity-not-executed-costs',selectedMeanReferenceR:sourceSafe?mean(closed.map(adjusted)):null,pairedOpportunities:paired.length,pairedDeltaReferenceRPerSourceOpportunity:sourceSafe?mean(paired.map(e=>(e.enrollment.assessments.find(a=>a.gate===gate)?.decision==='accept'?adjusted(e):0)-adjusted(e))):null};});
   return {gate,baselineOpportunities:population.length,selected:selected.length,rejected:known.length-selected.length,unavailable:population.length-known.length,featureCoverage:sourceSafe&&population.length?known.length/population.length:null,selectivity:sourceSafe&&population.length?selected.length/population.length:null,counts,closedDenominator:closed.length,targetCloseFraction:sourceSafe&&closed.length?targets/closed.length:null,wilson95HypotheticalIid:sourceSafe?studyWilsonReference(targets,closed.length):null,meanUnderlyingCloseReferenceR:sourceSafe?mean(closed.map(e=>e.grossCloseReferenceR!)):null,costSensitivity:costs};
  });return {partition,opportunities:population.length,arms:summaries};
 });
 const out={version:'chart-signal-study-v1',registryId:P.registryId,registrySignature,mode:'shadow',source:{id:manifest.sourceId,asOfMs:manifest.asOfMs,complete:sourceSafe,inputRows:Array.isArray(rows)?rows.length:0,uniqueRows:seenCases.size,duplicateRows:duplicates,invalidRows,unknownProtocolRows,referenceRows,recentTrialEvictions,recentOutcomeEvictions,bytes:byteCount,omittedRows:manifest.omittedRows??0},integrity:{status:sourceSafe?'valid-derived-scope':'incomplete-or-rejected',issues:[...issues].sort()},registration:{registeredAtMs:P.registeredAtMs,trainFrom:P.trainFrom,validationFrom:P.validationFrom,holdoutFrom:P.holdoutFrom,holdoutUntil:P.holdoutUntil,parameterTuningAllowed:false},sessions:{completedDates,minimumPerPartition:P.minimumCompletedDatesPerPartition,ready:sourceSafe&&Object.values(completedDates).every(v=>v.length>=P.minimumCompletedDatesPerPartition)},groups,episodes:list.map(e=>({id:e.enrollment.id,symbol:e.enrollment.ticket.identity.symbol,dateET:e.dateET,partition:e.partition,knownAtMs:e.enrollment.knownAtMs,deadlineMs:e.enrollment.ticket.deadlineMs,purged:e.purged,label:e.label,terminalCloseMs:e.terminalCloseMs,labelAvailableMs:e.labelAvailableMs,underlyingCloseReferenceR:e.grossCloseReferenceR,issues:[...e.issues].sort()})),inference:{status:'not-validated',effectiveIndependentSampleSize:null,clusterConfidenceInterval:null,multipleTrialSelectionControlled:false,calibratedProbability:null,promotionEnabled:false,reason:'Fixed gates and date partitions are software controls. Event dependence, selection, censoring and execution costs prevent a calibrated trading claim.'},optionPerformance:{status:'unavailable',missing:['contract-identity','entry-and-exit-quotes','sizes','executions','fees-and-slippage'],pnl:null},entryAuthorized:false,executionEnabled:false,markersEnabled:false,probability:null,fill:null,pnl:null};
 if(bytes(out)>P.maxReportBytes)throw Error('study-report-byte-capacity');return freeze(out);
}
export type ChartSignalStudyReport=ReturnType<typeof compareChartSignalStudy>;
