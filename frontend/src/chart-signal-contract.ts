import {ROUTE_BAR_MS,routeSequenceDeadlineMs,routeSequenceBarExpired,routeSequenceAdmissionExpired} from './route-sequence-policy';

/** Stage 1 foundation only: not connected to chart rendering, execution or persisted v1 histories. */
export const CHART_SIGNAL_CONTRACT_VERSION='chart-signal-contract-v1' as const;
export type ChartSignalState='candidate'|'confirmed'|'active'|'invalidated'|'reached'|'expired';
export type ChartSignalStrategy='session-intraday-v1'|'important-level-route-v1';
export interface ChartSignalIdentity {id:string;symbol:string;sessionKey:string;strategyId:ChartSignalStrategy;sourceEventKey:string}
export interface ChartSignalClock {sourceDataMs:number;publishedMs:number;availableMs:number}
export interface ChartSignalCoverage {
 name:string;unit:string;status:'available'|'partial'|'unavailable';numerator:number|null;denominator:number|null;
}
export interface ChartSignalProvenance {
 engine:string;engineVersion:string;frameId:string;evidenceKey:string;sourceFingerprint:string;
 coverage:readonly ChartSignalCoverage[];
}
/** Quality is an explicit source assessment, not a lifecycle state or a probability. */
export interface ChartSignalQuality {status:'qualified'|'partial'|'blocked'|'unknown';reasonCodes:readonly string[]}
export interface ChartSignalProposal extends ChartSignalClock {
 identity:ChartSignalIdentity;session:{openMs:number;closeMs:number};side:'CALL'|'PUT';
 impulseCloseMs:number;triggerCloseMs:number;deadlineMs:number;
 source:{kind:'verified-sequence'|'preparation'|'legacy-partial';state:'candidate'|'confirmed'|'sustained'|'partial';support:'complete'|'partial'|'missing'};
 entryReference:{price:number;atMs:number;kind:'observed-underlying-reference-not-fill'}|null;
 target:{price:number;name:string;firstAvailableMs:number;selectedMs:number}|null;
 invalidation:{price:number;method:'one-minute-close'}|null;
 quality:ChartSignalQuality;provenance:ChartSignalProvenance;reasonCodes:readonly string[];
}
export interface ChartSignalTicket extends ChartSignalProposal {
 version:typeof CHART_SIGNAL_CONTRACT_VERSION;confirmedAtMs:number;
 geometry:{direction:1|-1;risk:number;reward:number;rewardRisk:number;unit:'underlying-price-units'};
 resolution:'closed-one-minute-bar';deadlineRule:'min(impulse-close-plus-10m,session-close);exclusive-new-confirmation';
 entryAuthorized:false;executionEnabled:false;probability:null;fill:null;
}
interface EventBase {eventId:string;evaluatedMs:number}
interface ExistingEvent extends EventBase,ChartSignalClock {identity:ChartSignalIdentity;sourceFingerprint:string;reasonCodes:readonly string[]}
export type ChartSignalEvent=
 | (EventBase&{kind:'candidate';proposal:ChartSignalProposal})
 | (EventBase&{kind:'confirm';proposal:ChartSignalProposal})
 | (ExistingEvent&{kind:'observe'})
 | (ExistingEvent&{kind:'resolve-close';bar:{startMs:number;closeMs:number;close:number;intervalMs:60000}})
 | (ExistingEvent&{kind:'expire';completeThroughCloseMs:number});
export interface ChartSignalAudit {
 event:Readonly<ChartSignalEvent>;disposition:'accepted'|'rejected'|'duplicate';reasonCode:string;
 from:ChartSignalState|null;to:ChartSignalState|null;
}
export interface ChartSignalRecord {
 version:typeof CHART_SIGNAL_CONTRACT_VERSION;identity:Readonly<ChartSignalIdentity>;state:ChartSignalState;
 proposal:Readonly<ChartSignalProposal>;ticket:Readonly<ChartSignalTicket>|null;
 lastAcceptedEvaluationMs:number;lastAcceptedClock:Readonly<ChartSignalClock>;lastResolvedCloseMs:number|null;history:readonly Readonly<ChartSignalAudit>[];
 entryAuthorized:false;executionEnabled:false;probability:null;fill:null;
}
export interface ChartSignalProjection {accepted:boolean;reasonCode:string;signal:Readonly<ChartSignalRecord>|null;audit:Readonly<ChartSignalAudit>}

const clock=(v:unknown):v is number=>typeof v==='number'&&Number.isSafeInteger(v)&&v>0&&v<=8640000000000000;
const price=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v)&&v>0;
const text=(v:unknown):v is string=>typeof v==='string'&&v.length>0&&v.length<=512;
const codes=(v:unknown):v is readonly string[]=>Array.isArray(v)&&v.every(text);
const terminal=(v:ChartSignalState)=>v==='invalidated'||v==='reached'||v==='expired';
function freeze<T>(v:T):Readonly<T>{if(v&&typeof v==='object'&&!Object.isFrozen(v)){for(const child of Object.values(v))freeze(child);Object.freeze(v);}return v;}
function copy<T>(v:T):T{return structuredClone(v);}
function canonical(v:unknown):string {
 if(Array.isArray(v))return '['+v.map(canonical).join(',')+']';
 if(v&&typeof v==='object')return '{'+Object.entries(v).sort(([a],[b])=>a<b?-1:a>b?1:0).map(([k,x])=>JSON.stringify(k)+':'+canonical(x)).join(',')+'}';
 return JSON.stringify(v)??'undefined';
}
function identityValid(v:ChartSignalIdentity):boolean{return !!v&&[v.id,v.symbol,v.sessionKey,v.sourceEventKey].every(text)&&['session-intraday-v1','important-level-route-v1'].includes(v.strategyId);}
function sameIdentity(a:ChartSignalIdentity,b:ChartSignalIdentity):boolean{return !!b&&a.id===b.id&&a.symbol===b.symbol&&a.sessionKey===b.sessionKey&&a.strategyId===b.strategyId&&a.sourceEventKey===b.sourceEventKey;}
function clockIssue(v:ChartSignalClock,now:number):string|null {
 if(!v||![v.sourceDataMs,v.publishedMs,v.availableMs,now].every(clock))return 'invalid-clock';
 return v.sourceDataMs>v.publishedMs||v.publishedMs>v.availableMs||v.availableMs>now?'non-causal-clock':null;
}
function proposalIssue(p:ChartSignalProposal,now:number):string|null {
 if(!p||!identityValid(p.identity)||!p.session||![p.session.openMs,p.session.closeMs,p.impulseCloseMs,p.triggerCloseMs,p.deadlineMs].every(clock)||p.session.openMs>=p.session.closeMs||!['CALL','PUT'].includes(p.side))return 'invalid-identity-or-session';
 const timing=clockIssue(p,now);if(timing)return timing;
 if(p.impulseCloseMs<p.session.openMs+ROUTE_BAR_MS||p.triggerCloseMs<p.impulseCloseMs||p.triggerCloseMs>p.sourceDataMs||p.triggerCloseMs>p.session.closeMs||(p.impulseCloseMs-p.session.openMs)%ROUTE_BAR_MS!==0||(p.triggerCloseMs-p.session.openMs)%ROUTE_BAR_MS!==0)return 'unverified-closed-trigger';
 const deadline=Math.min(routeSequenceDeadlineMs(p.impulseCloseMs),p.session.closeMs);
 if(p.deadlineMs!==deadline)return 'inconsistent-deadline';
 if(!p.source||!['verified-sequence','preparation','legacy-partial'].includes(p.source.kind)||!['candidate','confirmed','sustained','partial'].includes(p.source.state)||!['complete','partial','missing'].includes(p.source.support))return 'invalid-source-assessment';
 if(!p.quality||!['qualified','partial','blocked','unknown'].includes(p.quality.status)||!codes(p.quality.reasonCodes)||!codes(p.reasonCodes))return 'missing-quality-or-reasons';
 const v=p.provenance;
 if(!v||![v.engine,v.engineVersion,v.frameId,v.evidenceKey,v.sourceFingerprint].every(text)||!Array.isArray(v.coverage)||!v.coverage.length)return 'missing-provenance-or-coverage';
 for(const c of v.coverage){
  if(!c||![c.name,c.unit].every(text)||!['available','partial','unavailable'].includes(c.status))return 'invalid-coverage';
  if(c.status==='unavailable'){if(c.numerator!==null||c.denominator!==null)return 'invalid-coverage';}
  else if(typeof c.numerator!=='number'||!Number.isFinite(c.numerator)||typeof c.denominator!=='number'||!Number.isFinite(c.denominator)||c.numerator<0||c.denominator<=0||c.numerator>c.denominator)return 'invalid-coverage';
 }
 const e=p.entryReference,t=p.target,i=p.invalidation;
 if(e!==null&&(!e||!price(e.price)||!clock(e.atMs)||e.kind!=='observed-underlying-reference-not-fill'||e.atMs<p.triggerCloseMs||e.atMs>p.sourceDataMs))return 'invalid-entry-reference';
 if(t!==null&&(!t||!price(t.price)||!text(t.name)||![t.firstAvailableMs,t.selectedMs].every(clock)||t.firstAvailableMs>p.triggerCloseMs||t.firstAvailableMs>t.selectedMs||t.selectedMs>p.availableMs))return 'unavailable-target';
 if(i!==null&&(!i||!price(i.price)||i.method!=='one-minute-close'))return 'invalid-invalidation';
 if(e&&t&&i){const d=p.side==='CALL'?1:-1,risk=d*(e.price-i.price),reward=d*(t.price-e.price);if(risk<=0||reward<=0||!Number.isFinite(reward/risk))return 'invalid-signed-geometry';}
 return null;
}
function confirmationIssue(p:ChartSignalProposal,now:number):string|null {
 if(p.source.kind!=='verified-sequence'||p.source.support!=='complete'||p.quality.status!=='qualified')return 'source-not-qualified';
 const expected=p.identity.strategyId==='session-intraday-v1'?'confirmed':'sustained';
 if(p.source.state!==expected||p.triggerCloseMs<=p.impulseCloseMs||p.provenance.coverage.some(c=>c.status!=='available'))return 'source-not-confirmed';
 if(routeSequenceAdmissionExpired(p.availableMs,p.deadlineMs)||routeSequenceAdmissionExpired(now,p.deadlineMs))return 'confirmation-deadline';
 if(!p.entryReference||!p.target||!p.invalidation)return 'missing-geometry';
 const d=p.side==='CALL'?1:-1,risk=d*(p.entryReference.price-p.invalidation.price),reward=d*(p.target.price-p.entryReference.price);
 return !Number.isFinite(risk)||!Number.isFinite(reward)||risk<=0||reward<=0||!Number.isFinite(reward/risk)?'invalid-signed-geometry':null;
}
function ticket(p:ChartSignalProposal,now:number):Readonly<ChartSignalTicket> {
 const d=p.side==='CALL'?1:-1,risk=d*(p.entryReference!.price-p.invalidation!.price),reward=d*(p.target!.price-p.entryReference!.price);
 return freeze({...copy(p),version:CHART_SIGNAL_CONTRACT_VERSION,confirmedAtMs:now,geometry:{direction:d as 1|-1,risk,reward,rewardRisk:reward/risk,unit:'underlying-price-units' as const},resolution:'closed-one-minute-bar' as const,deadlineRule:'min(impulse-close-plus-10m,session-close);exclusive-new-confirmation' as const,entryAuthorized:false as const,executionEnabled:false as const,probability:null,fill:null});
}
function firstEligibleClose(t:Readonly<ChartSignalTicket>):number {
 const known=Math.max(t.availableMs,t.confirmedAtMs);
 return t.session.openMs+(Math.floor((known-t.session.openMs)/ROUTE_BAR_MS)+1)*ROUTE_BAR_MS;
}

/**
 * Deterministic metadata projection. Explicit observations only; never reruns engines or reconstructs
 * skipped source fields. Confirmed means a qualified technical event; active means its scenario is
 * being observed. Neither is an order, position, executable entry, fill or win-probability estimate.
 * Rejected revisions/duplicates append an audit entry and preserve the original ticket/history.
 */
export function projectChartSignal(previous:Readonly<ChartSignalRecord>|null,event:ChartSignalEvent):Readonly<ChartSignalProjection> {
 const finish=(accepted:boolean,reasonCode:string,state=previous?.state??null,proposal=previous?.proposal,t=previous?.ticket??null,lastClose=previous?.lastResolvedCloseMs??null,disposition:ChartSignalAudit['disposition']=accepted?'accepted':'rejected'):Readonly<ChartSignalProjection>=>{
  const audit=freeze({event:copy(event),disposition,reasonCode,from:previous?.state??null,to:state});
  const c=event&&(event.kind==='candidate'||event.kind==='confirm')?event.proposal:event as ChartSignalClock;
  const signal=state&&proposal?freeze({version:CHART_SIGNAL_CONTRACT_VERSION,identity:copy(proposal.identity),state,proposal:copy(proposal),ticket:t,lastAcceptedEvaluationMs:accepted?event.evaluatedMs:previous!.lastAcceptedEvaluationMs,lastAcceptedClock:accepted?{sourceDataMs:c.sourceDataMs,publishedMs:c.publishedMs,availableMs:c.availableMs}:previous!.lastAcceptedClock,lastResolvedCloseMs:lastClose,history:[...(previous?.history??[]),audit],entryAuthorized:false as const,executionEnabled:false as const,probability:null,fill:null}):null;
  return freeze({accepted,reasonCode,signal,audit});
 };
 if(!event||!text(event.eventId)||!clock(event.evaluatedMs)||!['candidate','confirm','observe','resolve-close','expire'].includes(event.kind))return finish(false,'invalid-event');
 if(previous){
  if(previous.version!==CHART_SIGNAL_CONTRACT_VERSION)return finish(false,'foreign-contract');
  const prior=previous.history.find(v=>v.event.eventId===event.eventId);
  if(prior)return finish(false,canonical(prior.event)===canonical(event)?'duplicate-event':'event-revised',previous.state,previous.proposal,previous.ticket,previous.lastResolvedCloseMs,canonical(prior.event)===canonical(event)?'duplicate':'rejected');
  if(event.evaluatedMs<previous.lastAcceptedEvaluationMs)return finish(false,'backward-evaluation');
  const c=event.kind==='candidate'||event.kind==='confirm'?event.proposal:event,q=previous.lastAcceptedClock;
  if(c&&(c.sourceDataMs<q.sourceDataMs||c.publishedMs<q.publishedMs||c.availableMs<q.availableMs))return finish(false,'backdated-source');
 }
 if(event.kind==='candidate'||event.kind==='confirm'){
  const p=event.proposal,issue=proposalIssue(p,event.evaluatedMs);if(issue)return finish(false,issue);
  if(previous){
   if(!sameIdentity(previous.identity,p.identity))return finish(false,'foreign-signal');
   if(previous.ticket)return finish(false,'immutable-ticket-revision');
   if(terminal(previous.state))return finish(false,'terminal-state');
   const q=previous.proposal;
   if(p.side!==q.side||p.session.openMs!==q.session.openMs||p.session.closeMs!==q.session.closeMs||p.impulseCloseMs!==q.impulseCloseMs||p.deadlineMs!==q.deadlineMs)return finish(false,'episode-revised');
   if(p.availableMs<q.availableMs||p.triggerCloseMs<q.triggerCloseMs||p.sourceDataMs<q.sourceDataMs)return finish(false,'backdated-source');
  }
  if(event.kind==='candidate')return finish(true,'candidate-observed','candidate',p,null);
  const confirmation=confirmationIssue(p,event.evaluatedMs);if(confirmation)return finish(false,confirmation);
  return finish(true,'technical-confirmation','confirmed',p,ticket(p,event.evaluatedMs));
 }
 if(!previous)return finish(false,'missing-signal');
 if(!sameIdentity(previous.identity,event.identity))return finish(false,'foreign-signal');
 if(event.sourceFingerprint!==(previous.ticket??previous.proposal).provenance.sourceFingerprint)return finish(false,'frozen-source-revised');
 const timing=clockIssue(event,event.evaluatedMs);if(timing)return finish(false,timing);
 if(!codes(event.reasonCodes))return finish(false,'missing-reasons');
 if(event.availableMs<previous.proposal.availableMs)return finish(false,'backdated-source');
 if(terminal(previous.state))return finish(false,'terminal-state');
 if(event.kind==='expire'){
  const deadline=previous.proposal.deadlineMs;
  if(!clock(event.completeThroughCloseMs)||event.completeThroughCloseMs<deadline||event.completeThroughCloseMs>event.sourceDataMs||event.evaluatedMs<=deadline)return finish(false,'unverified-expiry-watermark');
  if(previous.ticket){
   const t=previous.ticket,finalClose=t.session.openMs+Math.floor((deadline-t.session.openMs)/ROUTE_BAR_MS)*ROUTE_BAR_MS;
   if(firstEligibleClose(t)<=finalClose&&(previous.lastResolvedCloseMs===null||previous.lastResolvedCloseMs<finalClose))return finish(false,'expiry-without-complete-close-coverage');
  }
  return finish(true,'deadline-expired','expired');
 }
 const t=previous.ticket;if(!t)return finish(false,'not-confirmed');
 if(event.kind==='observe'){
  if(event.availableMs>=t.deadlineMs||event.evaluatedMs>=t.deadlineMs)return finish(false,'observation-after-deadline');
  return finish(true,'scenario-in-observation','active');
 }
 const b=event.bar;
 if(!b||!clock(b.startMs)||b.intervalMs!==ROUTE_BAR_MS||b.closeMs!==b.startMs+ROUTE_BAR_MS||(b.startMs-t.session.openMs)%ROUTE_BAR_MS!==0||!price(b.close)||b.closeMs>event.sourceDataMs)return finish(false,'unverified-one-minute-close');
 if(b.closeMs<=t.availableMs||b.closeMs<=(previous.lastResolvedCloseMs??0))return finish(false,'backdated-close');
 // Every eligible close must be processed. A gap could conceal the first invalidation or target.
 const firstClose=firstEligibleClose(t);
 const expectedClose=previous.lastResolvedCloseMs===null?firstClose:previous.lastResolvedCloseMs+ROUTE_BAR_MS;
 if(b.closeMs!==expectedClose)return finish(false,'missing-close-continuity');
 // Expiry wins after the fixed window; a late bar cannot manufacture a target/invalidation.
 if(routeSequenceBarExpired(b.closeMs,t.deadlineMs))return finish(true,'deadline-expired','expired',previous.proposal,t,b.closeMs);
 const d=t.geometry.direction;
 if(d*(b.close-t.target!.price)>=0)return finish(true,'target-reached-by-close','reached',previous.proposal,t,b.closeMs);
 if(d*(b.close-t.invalidation!.price)<0)return finish(true,'invalidated-by-close','invalidated',previous.proposal,t,b.closeMs);
 return finish(true,'scenario-in-observation','active',previous.proposal,t,b.closeMs);
}
