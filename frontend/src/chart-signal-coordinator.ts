import {CHART_SIGNAL_CONTRACT_VERSION,projectChartSignal,type ChartSignalRecord,type ChartSignalProposal,type ChartSignalTicket,type ChartSignalState,type ChartSignalCoverage,type ChartSignalStrategy} from './chart-signal-contract';
import {validChartSignalDiagnostics,type ChartSignalDiagnosticsReport} from './chart-signal-diagnostics';
import type {RouteOpportunitySource,FrozenRouteOpportunity} from './route-opportunity-source';
import type {Candle} from './model';

/** Versioned research policy, not an independent vote, trading order or probability model. */
export const CHART_SIGNAL_COORDINATOR_POLICY=Object.freeze({version:'chart-signal-coordinator-v1',mode:'shadow',strategies:['important-level-route-v1','session-intraday-v1'] as const,legacyAdmission:'candidate-only',intradayAdmission:'existing-confirmed-source-only',geometryReference:'closed-trigger-underlying-not-fill',oneActiveEpisodePerStrategy:true,retirement:"next-source-episode-after-fixed-deadline;unresolved-outcome-censored",options:'existing-source-context-only',reversalAbsorption:'separate-hypothesis-no-vote',receptionCoverageRequired:false,maxHistoryPerEpisode:32,markersEnabled:false,executionEnabled:false} as const);
type Strategy=ChartSignalStrategy;
type Audit={kind:string;disposition:string;reasonCode:string}|null;
export interface CoordinatedChartSignal {
 id:string|null;strategyId:Strategy;family:'continuation';sourcePhase:string;sourceKey:string|null;
 state:ChartSignalState|'waiting';ticket:Readonly<ChartSignalTicket>|null;lastResolvedCloseMs:number|null;
 reasonCodes:readonly string[];lastAudit:Audit;revisionObserved:boolean;
}
export interface ChartSignalCoordinatorReport {
 version:'chart-signal-coordinator-v1';mode:'shadow';symbol:string;sessionKey:string|null;
 evaluatedMs:number|null;publishedMs:number|null;connectionStartedMs:number|null;
 status:'observed'|'waiting';signals:readonly Readonly<CoordinatedChartSignal>[];
 hypotheses:readonly [{family:'continuation';role:'source-qualified-only'},{family:'reversal-absorption';role:'separate-context-not-an-extra-vote'}];
 evidencePolicy:{price:'closed-one-minute-sequence';cvd:'existing-source-five-minute-shares';options:'context-no-additional-vote';gexSelection:'not-an-input';receptionPercent:null;independentVotes:null};
 semanticKey:string;entryAuthorized:false;executionEnabled:false;markersEnabled:false;probability:null;fill:null;
}
export interface ChartSignalCoordinatorInput {
 symbol:string;at:number;publishedMs:number|null;startedMs:number|null;active:boolean;
 calendar:{date:string;openMs:number;closeMs:number}|null;bars:readonly Candle[];
 source:Readonly<RouteOpportunitySource>|undefined;diagnostics:Readonly<ChartSignalDiagnosticsReport>|null|undefined;
}
const strategies=CHART_SIGNAL_COORDINATOR_POLICY.strategies;
const clock=(v:unknown):v is number=>typeof v==='number'&&Number.isSafeInteger(v)&&v>0&&v<=8640000000000000;
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v);
const positive=(v:unknown):v is number=>finite(v)&&v>0;
const text=(v:unknown):v is string=>typeof v==='string'&&v.length>0&&v.length<=512;
const terminal=(state:ChartSignalState)=>['invalidated','reached','expired'].includes(state);
const day=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
function freeze<T>(v:T):Readonly<T>{if(v&&typeof v==='object'&&!Object.isFrozen(v)){Object.values(v).forEach(freeze);Object.freeze(v);}return v;}
function barKey(b:Candle){return JSON.stringify([b.timestampMs,b.open,b.high,b.low,b.close,b.volume]);}
function minuteMap(bars:readonly Candle[],published:number){
 const map=new Map<number,Candle>(),conflicts=new Set<number>();
 for(const b of bars){
  if(!b||!clock(b.timestampMs)||b.timestampMs+60000>published||![b.open,b.high,b.low,b.close].every(positive)||!finite(b.volume)||b.volume<0||b.high<Math.max(b.open,b.close)||b.low>Math.min(b.open,b.close)||b.low>b.high)continue;
  const old=map.get(b.timestampMs);if(old&&barKey(old)!==barKey(b))conflicts.add(b.timestampMs);else map.set(b.timestampMs,b);
 }
 return {map,conflicts};
}
function identity(symbol:string,session:string,strategy:Strategy,key:string){return {id:JSON.stringify([symbol,session,strategy,key]),symbol,sessionKey:session,strategyId:strategy,sourceEventKey:key};}
function fingerprint(e:FrozenRouteOpportunity){return JSON.stringify([e.strategyId,e.sourceResultKey,e.origin.startedMs,e.signal.availableMs,e.impulseBar.endMs,e.triggerBar.endMs,e.triggerBar.close,e.anchor.price,e.anchor.firstObservedMs,e.target.price,e.target.firstObservedMs,e.target.selectedMs,e.stop]);}
function requiredCoverage(input:ChartSignalCoordinatorInput,e:FrozenRouteOpportunity):ChartSignalCoverage[]{
 const row=(name:string):ChartSignalCoverage=>{
  const r=input.diagnostics?.coverage.rows.find(v=>v.kind===name);
  const sourceComplete=name!=='stock-share-classification'||!!r&&r.complete===true&&r.percent!==null&&r.percent>=70&&r.numerator!==null&&r.denominator!==null&&r.denominator>0&&Math.abs(r.percent-100*r.numerator/r.denominator)<=1e-8;
  return r&&r.status==='available'&&sourceComplete&&r.numerator!==null&&r.denominator!==null&&r.denominator>0&&r.numerator>0&&r.numerator<=r.denominator?{name,unit:r.unit,status:'available',numerator:r.numerator,denominator:r.denominator}:{name,unit:r?.unit??'events',status:'unavailable',numerator:null,denominator:null};
 };
 const {map,conflicts}=minuteMap(input.bars,input.publishedMs!);let count=0;
 const expected=(e.triggerBar.endMs-e.impulseBar.startMs)/60000;
 if(Number.isSafeInteger(expected)&&expected>0&&expected<=20)for(let t=e.impulseBar.startMs;t<e.triggerBar.endMs;t+=60000)if(map.has(t)&&!conflicts.has(t))count++;
 const sequence:ChartSignalCoverage={name:'source-sequence-closed-minutes',unit:'minutes',status:count===expected&&expected>0?'available':'partial',numerator:count,denominator:expected>0&&expected<=20?expected:1};
 if(sequence.numerator!>sequence.denominator!)return [row('price-clock'),{...sequence,status:'unavailable',numerator:null,denominator:null},row('stock-share-classification')];
 return [row('price-clock'),sequence,row('stock-share-classification')];
}
function proposal(input:ChartSignalCoordinatorInput,e:FrozenRouteOpportunity,session:string):ChartSignalProposal {
 const coverage=requiredCoverage(input,e),complete=coverage.every(v=>v.status==='available');
 const qualified=e.strategyId==='session-intraday-v1'&&e.sourceState==='confirmed'&&complete;
 const t=e.target,verifiedTarget=positive(t.price)&&text(t.name)&&clock(t.firstObservedMs)&&clock(t.selectedMs);
 return {identity:identity(input.symbol,session,e.strategyId,e.sourceResultKey),session:{openMs:input.calendar!.openMs,closeMs:input.calendar!.closeMs},side:e.direction===1?'CALL':'PUT',sourceDataMs:input.publishedMs!,publishedMs:input.publishedMs!,availableMs:e.signal.availableMs,impulseCloseMs:e.impulseBar.endMs,triggerCloseMs:e.triggerBar.endMs,deadlineMs:Math.min(e.impulseBar.endMs+600000,input.calendar!.closeMs),
  source:{kind:qualified?'verified-sequence':e.strategyId==='important-level-route-v1'?'legacy-partial':'preparation',state:e.sourceState,support:complete?'complete':'partial'},
  entryReference:{price:e.triggerBar.close,atMs:e.triggerBar.endMs,kind:'observed-underlying-reference-not-fill'},
  target:verifiedTarget?{price:t.price,name:t.name!,firstAvailableMs:t.firstObservedMs!,selectedMs:t.selectedMs!}:null,invalidation:{price:e.stop,method:'one-minute-close'},
  quality:{status:qualified?'qualified':'partial',reasonCodes:qualified?[]:[e.strategyId==='important-level-route-v1'?'legacy-candidate-only':'required-source-evidence-unavailable']},
  provenance:{engine:e.strategyId,engineVersion:e.version,frameId:JSON.stringify([input.symbol,input.startedMs,input.publishedMs]),evidenceKey:JSON.stringify([e.strategyId,e.sourceResultKey,e.signal.availableMs]),sourceFingerprint:fingerprint(e),coverage},reasonCodes:['observed-underlying-reference-not-fill','required-evidence-not-market-reception']};
}
interface Slot {record:Readonly<ChartSignalRecord>;fingerprint:string;resolved:Map<number,string>;revision:boolean}
/** One instance per existing route reader, shared by both chart views. No feed, DOM, timer or broker. */
export class ChartSignalCoordinator {
 private epoch='';private lastAt=0;private lastPublished=0;private slots=new Map<Strategy,Slot>();private latest?:Readonly<ChartSignalCoordinatorReport>;
 observe(input:ChartSignalCoordinatorInput):Readonly<ChartSignalCoordinatorReport>{
  const validClock=clock(input.at)&&clock(input.publishedMs)&&clock(input.startedMs)&&input.startedMs<=input.publishedMs&&input.publishedMs<=input.at;
  const date=clock(input.at)?day.format(input.at):null,session=validClock?`${input.symbol}:${date}:${input.startedMs}`:null;
  const d=input.diagnostics,s=input.source,c=input.calendar;
  const associated=validClock&&text(input.symbol)&&!!d&&validChartSignalDiagnostics(d)&&d.symbol===input.symbol&&d.sessionKey===session&&d.evaluatedMs===input.at&&d.publishedMs===input.publishedMs&&d.connectionStartedMs===input.startedMs&&s?.version==='route-opportunity-source-v1'&&s.symbol===input.symbol&&s.at===input.at&&s.publishedMs===input.publishedMs&&s.startedMs===input.startedMs&&s.market.symbol===input.symbol&&s.market.session===session&&s.market.at===input.at;
  const rth=associated&&input.active&&input.at-input.publishedMs!<=3000&&c?.date===date&&clock(c.openMs)&&clock(c.closeMs)&&c.openMs<c.closeMs&&day.format(c.openMs)===date&&day.format(c.closeMs-1)===date&&input.at>=c.openMs&&input.at<c.closeMs;
  const backward=this.epoch===session&&(input.at<this.lastAt||input.publishedMs!<this.lastPublished);
  if(associated&&!backward&&session!==this.epoch){this.slots.clear();this.epoch=session!;this.lastAt=0;this.lastPublished=0;}
  const rows:CoordinatedChartSignal[]=[];
  for(const strategy of strategies){
   const source=s?.strategies.find(v=>v.strategyId===strategy),diagnostic=d?.strategies.find(v=>v.strategyId===strategy),reason:string[]=[];
   let slot=associated&&!backward?this.slots.get(strategy):undefined;
   if(!associated)reason.push('source-association-unavailable');else if(backward)reason.push('backward-source-observation');else if(!rth)reason.push('outside-current-rth');
   if(slot){
    const bars=minuteMap(input.bars,input.publishedMs!);
    for(const [t,original]of slot.resolved){const b=bars.map.get(t);if(b&&barKey(b)!==original||bars.conflicts.has(t))slot.revision=true;}
    if(slot.revision)reason.push('resolved-close-revised-not-recomputed');
    const record=slot.record,ticket=record.ticket;
    if(ticket&&!terminal(record.state)&&!slot.revision){
     let next=record.lastResolvedCloseMs===null?ticket.session.openMs+(Math.floor((Math.max(ticket.availableMs,ticket.confirmedAtMs)-ticket.session.openMs)/60000)+1)*60000:record.lastResolvedCloseMs+60000;
     let steps=0;
     while(next<=input.publishedMs!&&next<=ticket.session.closeMs&&steps++<20&&!terminal(slot.record.state)){
      const b=bars.map.get(next-60000);
      if(!b||bars.conflicts.has(next-60000)){reason.push('resolution-close-unavailable');break;}
      const event={eventId:record.identity.id+':close:'+next,kind:'resolve-close' as const,evaluatedMs:input.at,identity:record.identity,sourceFingerprint:slot.fingerprint,sourceDataMs:input.publishedMs!,publishedMs:input.publishedMs!,availableMs:input.at,reasonCodes:[],bar:{startMs:b.timestampMs,closeMs:next,close:b.close,intervalMs:60000 as const}};
      const p=projectChartSignal(slot.record,event);if(!p.accepted||!p.signal){reason.push(p.reasonCode);break;}
      slot.record=p.signal;slot.resolved.set(b.timestampMs,barKey(b));next+=60000;
     }
    }
    if(!terminal(slot.record.state)&&input.at>slot.record.proposal.deadlineMs){
     const end=Math.max(0,...bars.map.keys())+60000;
     const p=projectChartSignal(slot.record,{eventId:record.identity.id+':expiry:'+end,kind:'expire',evaluatedMs:input.at,identity:record.identity,sourceFingerprint:slot.fingerprint,sourceDataMs:input.publishedMs!,publishedMs:input.publishedMs!,availableMs:input.at,reasonCodes:[],completeThroughCloseMs:end});
     if(p.accepted&&p.signal)slot.record=p.signal;else reason.push(p.reasonCode);
    }
    if(source?.current.key&&slot.record.identity.sourceEventKey!==source.current.key){
     if(slot.record.ticket&&!terminal(slot.record.state)&&input.at<=slot.record.ticket.deadlineMs)reason.push('existing-episode-still-in-observation');else {if(slot.record.ticket&&!terminal(slot.record.state))reason.push('prior-episode-ended-with-unverified-close-coverage');this.slots.delete(strategy);slot=undefined;}
    }
   }
   const event=source?.event;
   if(associated&&!backward&&rth&&event&&!slot){
    const bars=minuteMap(input.bars,input.publishedMs!),impulse=bars.map.get(event.impulseBar.startMs),trigger=bars.map.get(event.triggerBar.startMs);
    if(event.origin.symbol!==input.symbol||event.origin.session!==session||event.origin.startedMs!==input.startedMs||event.origin.marketEvidenceKey!==s!.market.key||event.sourceResultKey!==source!.current.key||event.strategyId!==strategy||event.direction!==source!.current.direction||event.signal.session!==session||![1,-1].includes(event.direction))reason.push('foreign-source-event');
    else if(event.signal.availableMs!==input.at||event.origin.publishedMs!==input.publishedMs)reason.push('source-event-not-new-no-backfill');
    else if(event.origin.evaluatedMs!==input.at||!impulse||!trigger||bars.conflicts.has(impulse.timestampMs)||bars.conflicts.has(trigger.timestampMs)||impulse.close!==event.impulseBar.close||trigger.close!==event.triggerBar.close||event.impulseBar.endMs!==impulse.timestampMs+60000||event.triggerBar.endMs!==trigger.timestampMs+60000||event.anchor.firstObservedMs>event.impulseBar.startMs||event.anchor.price!==source!.current.level||event.target.price!==source!.current.target||event.stop!==source!.current.stop)reason.push('source-price-proof-incompatible');
    else{
     const p=proposal(input,event,session!),candidate=projectChartSignal(null,{eventId:p.identity.id+':candidate',kind:'candidate',evaluatedMs:input.at,proposal:p});
     if(candidate.accepted&&candidate.signal){
      slot={record:candidate.signal,fingerprint:p.provenance.sourceFingerprint,resolved:new Map(),revision:false};this.slots.set(strategy,slot);
      if(strategy==='session-intraday-v1'&&diagnostic?.phase==='confirmed'&&source!.current.state==='confirmed'){
       const confirmed=projectChartSignal(slot.record,{eventId:p.identity.id+':confirm',kind:'confirm',evaluatedMs:input.at,proposal:p});
       if(confirmed.accepted&&confirmed.signal)slot.record=confirmed.signal;else reason.push(confirmed.reasonCode);
      }else reason.push('legacy-candidate-only');
     }else reason.push(candidate.reasonCode);
    }
   }
   if(slot?.record.ticket&&event&&event.sourceResultKey===slot.record.identity.sourceEventKey&&fingerprint(event)!==slot.fingerprint){slot.revision=true;reason.push('frozen-source-revised-not-recomputed');}
   if(!slot&&diagnostic?.phase==='candidate')reason.push('awaiting-existing-source-confirmation');
   if(strategy==='important-level-route-v1'&&slot?.record.state==='candidate')reason.push('legacy-candidate-only');
   const record=slot?.record,last=record?.history.at(-1),key=source?.current.key||null;
   rows.push({id:record?.identity.id??(associated&&key?identity(input.symbol,session!,strategy,key).id:null),strategyId:strategy,family:'continuation',sourcePhase:diagnostic?.phase??'waiting',sourceKey:key,state:record?.state??(associated&&diagnostic?.phase==='candidate'?'candidate':'waiting'),ticket:record?.ticket??null,lastResolvedCloseMs:record?.lastResolvedCloseMs??null,reasonCodes:[...new Set(reason)].sort(),lastAudit:last?{kind:last.event.kind,disposition:last.disposition,reasonCode:last.reasonCode}:null,revisionObserved:slot?.revision??false});
  }
  if(associated&&!backward){this.lastAt=input.at;this.lastPublished=input.publishedMs!;}
  const semanticKey=JSON.stringify(rows.map(v=>[v.id,v.strategyId,v.sourcePhase,v.state,v.ticket?.provenance.sourceFingerprint??null,v.reasonCodes,v.lastAudit?.reasonCode,v.revisionObserved]));
  const report:ChartSignalCoordinatorReport={version:'chart-signal-coordinator-v1',mode:'shadow',symbol:input.symbol,sessionKey:session,evaluatedMs:clock(input.at)?input.at:null,publishedMs:clock(input.publishedMs)?input.publishedMs:null,connectionStartedMs:clock(input.startedMs)?input.startedMs:null,status:associated&&!backward?'observed':'waiting',signals:rows,hypotheses:[{family:'continuation',role:'source-qualified-only'},{family:'reversal-absorption',role:'separate-context-not-an-extra-vote'}],evidencePolicy:{price:'closed-one-minute-sequence',cvd:'existing-source-five-minute-shares',options:'context-no-additional-vote',gexSelection:'not-an-input',receptionPercent:null,independentVotes:null},semanticKey,entryAuthorized:false,executionEnabled:false,markersEnabled:false,probability:null,fill:null};
  this.latest=freeze(report);
  return this.latest;
 }
 snapshot(symbol:string,now:number){const r=this.latest;return clock(now)&&r?.symbol===symbol&&r.evaluatedMs!==null&&r.evaluatedMs<=now&&now-r.evaluatedMs<=3000?r:undefined;}
}
/** Bounded optional ledger envelope; never changes the legacy case admission or recovery. */
export function validChartSignalCoordinatorReport(value:unknown):value is Readonly<ChartSignalCoordinatorReport>{
 try{
  const r=value as ChartSignalCoordinatorReport;if(!r||r.version!=='chart-signal-coordinator-v1'||r.mode!=='shadow'||!text(r.symbol)||!(r.sessionKey===null||text(r.sessionKey))||![r.evaluatedMs,r.publishedMs,r.connectionStartedMs].every(v=>v===null||clock(v))||r.entryAuthorized!==false||r.executionEnabled!==false||r.markersEnabled!==false||r.probability!==null||r.fill!==null||!['observed','waiting'].includes(r.status)||!Array.isArray(r.signals)||r.signals.length!==2||new Set(r.signals.map(v=>v.strategyId)).size!==2)return false;
  if(JSON.stringify(r).length>16384||r.semanticKey!==JSON.stringify(r.signals.map(v=>[v.id,v.strategyId,v.sourcePhase,v.state,v.ticket?.provenance.sourceFingerprint??null,v.reasonCodes,v.lastAudit?.reasonCode,v.revisionObserved])))return false;
  if(r.status==='observed'&&(!clock(r.evaluatedMs)||!clock(r.publishedMs)||!clock(r.connectionStartedMs)||r.connectionStartedMs>r.publishedMs||r.publishedMs>r.evaluatedMs||r.sessionKey!==`${r.symbol}:${day.format(r.evaluatedMs)}:${r.connectionStartedMs}`))return false;
  for(const row of r.signals){
   if(!strategies.includes(row.strategyId)||row.family!=='continuation'||!['waiting','candidate','confirmed','active','invalidated','reached','expired'].includes(row.state)||!(row.id===null||text(row.id))||!(row.sourceKey===null||text(row.sourceKey))||!Array.isArray(row.reasonCodes)||row.reasonCodes.length>32||row.reasonCodes.some((c:unknown)=>!text(c))||typeof row.revisionObserved!=='boolean'||!(row.lastResolvedCloseMs===null||clock(row.lastResolvedCloseMs)))return false;
   const t=row.ticket;if(t){
    if(r.status!=='observed'||row.state==='waiting'||row.state==='candidate')return false;
    if(row.strategyId!=='session-intraday-v1'||t.version!==CHART_SIGNAL_CONTRACT_VERSION||t.geometry.unit!=='underlying-price-units'||t.resolution!=='closed-one-minute-bar'||t.entryAuthorized!==false||t.executionEnabled!==false||t.probability!==null||t.fill!==null||t.identity.id!==row.id||t.identity.symbol!==r.symbol||t.identity.sessionKey!==r.sessionKey||t.source.kind!=='verified-sequence'||t.source.state!=='confirmed'||t.quality.status!=='qualified'||!t.entryReference||!t.target||!t.invalidation)return false;
    const dir=t.side==='CALL'?1:t.side==='PUT'?-1:0,risk=dir*(t.entryReference.price-t.invalidation.price),reward=dir*(t.target.price-t.entryReference.price);
    if(!dir||!positive(risk)||!positive(reward)||t.geometry.direction!==dir||t.geometry.risk!==risk||t.geometry.reward!==reward||t.geometry.rewardRisk!==reward/risk||t.confirmedAtMs>=t.deadlineMs||t.identity.strategyId!==row.strategyId)return false;
    if(!projectChartSignal(null,{eventId:'validate-ticket',kind:'confirm',evaluatedMs:t.confirmedAtMs,proposal:t}).accepted)return false;
   }else if(['confirmed','active','invalidated','reached'].includes(row.state))return false;
  }
  return r.hypotheses?.[1]?.role==='separate-context-not-an-extra-vote'&&r.evidencePolicy?.independentVotes===null&&r.evidencePolicy?.receptionPercent===null&&r.evidencePolicy?.gexSelection==='not-an-input';
 }catch{return false;}
}
