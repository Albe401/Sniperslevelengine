import type {ChartSignalCoverageReport,ChartSignalCoverageKind} from './chart-signal-coverage';
import type {RouteResult} from './route-engine';
import type {RouteCvdSupport} from './route-cvd';
import type {RouteStructure} from './route-level-context';
import type {IntradayRouteRead} from './session-intraday-route';

/** Passive source explanation only. No engine evaluation, admission, journal or UI. */
export const CHART_SIGNAL_DIAGNOSTICS_VERSION='chart-signal-diagnostics-v1' as const;
export type ChartSignalDiagnosticPhase='waiting'|'candidate'|'confirmed'|'blocked'|'invalidated'|'reached'|'expired';
export type ChartSignalDiagnosticCategory='actual-veto'|'dependent-sequence'|'context-unavailable'|'source-terminal';
export interface ChartSignalDiagnosticReason {category:ChartSignalDiagnosticCategory;code:string}
export interface ChartSignalStrategyDiagnostic {
 strategyId:'important-level-route-v1'|'session-intraday-v1';key:string|null;
 phase:ChartSignalDiagnosticPhase;rawSourceState:string;rawResultState:RouteResult['state'];side:'CALL'|'PUT'|null;
 reasonCodes:readonly string[];reasons:readonly Readonly<ChartSignalDiagnosticReason>[];
}
export interface ChartSignalDiagnosticsReport {
 version:typeof CHART_SIGNAL_DIAGNOSTICS_VERSION;symbol:string;sessionKey:string|null;
 evaluatedMs:number|null;publishedMs:number|null;connectionStartedMs:number|null;
 coverage:Readonly<ChartSignalCoverageReport>;strategies:readonly Readonly<ChartSignalStrategyDiagnostic>[];semanticKey:string;
 confirmationOrigin:'existing-source-only';entryAuthorized:false;executionEnabled:false;probability:null;admissionModified:false;signalsCreated:false;
}
export interface ChartSignalDiagnosticsInput {
 coverage:Readonly<ChartSignalCoverageReport>;
 important:{result:Readonly<RouteResult>;assessmentQuality:'pending'|'partial'|'aligned'|'opposed';cvd:Readonly<RouteCvdSupport>|null;structure:Readonly<RouteStructure>|null};
 intraday:Pick<IntradayRouteRead,'state'|'result'|'side'|'blockCodes'>;
}
const KINDS:readonly ChartSignalCoverageKind[]=['price-clock','closed-minute-continuity','stock-share-classification','option-contract-classification','option-premium-classification','option-premium-availability','stock-reception','option-reception'];
const SOURCE_STATES=['waiting','range','recovery','test','sustained','invalidated','reached'];
const INTRADAY_STATES=['waiting','candidate','confirmed','blocked','invalidated','reached'];
const PHASES=['waiting','candidate','confirmed','blocked','invalidated','reached','expired'];
const CATEGORIES=['actual-veto','dependent-sequence','context-unavailable','source-terminal'];
const sorted=(codes:readonly string[])=>[...new Set(codes)].sort();
const safeClock=(value:unknown):value is number=>typeof value==='number'&&Number.isSafeInteger(value)&&value>0&&value<=8640000000000000;
const maybeClock=(value:unknown)=>value===null||safeClock(value);
const machineCode=(value:unknown):value is string=>typeof value==='string'&&/^[a-z][a-z0-9-]{0,95}$/.test(value);
const codes=(value:unknown):value is string[]=>Array.isArray(value)&&value.length<=64&&value.every(machineCode)&&new Set(value).size===value.length;
function freeze<T>(value:T):Readonly<T>{if(value&&typeof value==='object'&&!Object.isFrozen(value)){for(const child of Object.values(value))freeze(child);Object.freeze(value);}return value;}
function reasons(){const rows:ChartSignalDiagnosticReason[]=[];return {rows,add(category:ChartSignalDiagnosticCategory,code:string){if(!rows.some(row=>row.category===category&&row.code===code))rows.push({category,code});}};}
function terminal(result:Readonly<RouteResult>):ChartSignalDiagnosticPhase|null {
 return result.state==='invalidated'?(result.terminalCause==='expired'?'expired':'invalidated'):result.state==='reached'?'reached':null;
}
function finish(row:Omit<ChartSignalStrategyDiagnostic,'reasonCodes'>):ChartSignalStrategyDiagnostic {
 row.reasons=[...row.reasons].sort((a,b)=>a.category.localeCompare(b.category)||a.code.localeCompare(b.code));
 return {...row,reasonCodes:sorted(row.reasons.map(reason=>reason.code))};
}
function contextReasons(coverage:Readonly<ChartSignalCoverageReport>,out:ReturnType<typeof reasons>){
 for(const code of coverage.reasonCodes)out.add('context-unavailable',code);
 for(const row of coverage.rows)if(row.status!=='available')out.add('context-unavailable',row.kind+'-'+row.status);
}
function importantDiagnostic(input:ChartSignalDiagnosticsInput):ChartSignalStrategyDiagnostic {
 const {result,assessmentQuality,cvd,structure}=input.important,out=reasons();contextReasons(input.coverage,out);
 const end=terminal(result),phase=end??(['recovery','test','sustained'].includes(result.state)?'candidate':'waiting');
 if(end)out.add('source-terminal',end==='expired'?'sequence-expired':end==='reached'?'reference-reached':'source-invalidated');
 else{
  if(result.state==='recovery')out.add('dependent-sequence','price-test-pending');
  else if(result.state==='test')out.add('dependent-sequence','price-continuation-pending');
  else if(result.state==='sustained')out.add('dependent-sequence','legacy-candidate-no-qualified-admission');
  else out.add('dependent-sequence','no-active-price-sequence');
  if(result.state==='test'&&structure?.requiresAdditionalTest)out.add('dependent-sequence','additional-price-continuation-pending');
  // These explain unusable support; legacy observation has no new admission veto.
  if(assessmentQuality!=='aligned')out.add('context-unavailable','important-assessment-'+assessmentQuality);
  if(!cvd||cvd.status!=='aligned')out.add('context-unavailable','important-cvd-'+(cvd?.status??'unavailable'));
  if(structure?.quality!=='supported')out.add('context-unavailable','important-backing-'+(structure?.quality??'unavailable'));
  if(input.coverage.existingConsumer.flow?.signal==null)out.add('context-unavailable','option-flow-context-unavailable');
  if(input.coverage.existingConsumer.cvd?.signal==null)out.add('context-unavailable','stock-cvd-context-unavailable');
 }
 return finish({strategyId:'important-level-route-v1',key:result.key||null,phase,rawSourceState:result.state,rawResultState:result.state,side:result.direction===1?'CALL':result.direction===-1?'PUT':null,reasons:out.rows});
}
function intradayDiagnostic(input:ChartSignalDiagnosticsInput):ChartSignalStrategyDiagnostic {
 const source=input.intraday,{result}=source,out=reasons();contextReasons(input.coverage,out);
 const end=terminal(result),phase=end??source.state;
 const given=sorted(source.blockCodes??[]);
 if(end){
  out.add('source-terminal',end==='expired'?'sequence-expired':end==='reached'?'reference-reached':'source-invalidated');
  for(const code of given)out.add('source-terminal',code);
 }else if(source.state==='blocked'){
  if(given.length)for(const code of given)out.add('actual-veto',code);
  else out.add('context-unavailable','source-block-codes-unavailable');
 }else if(source.state==='candidate'){
  for(const code of given)out.add('dependent-sequence',code);
  if(!given.length)out.add('dependent-sequence','price-sequence-pending');
 }else if(source.state==='waiting'){
  for(const code of given)out.add(code==='no-candidate-yet'||code==='history-revised'?'dependent-sequence':'context-unavailable',code);
  if(!given.length)out.add('dependent-sequence','no-active-price-sequence');
 }else if(given.length){
  // Preserve anomalous source explanations without making a new veto from them.
  out.add('context-unavailable','unexpected-source-reason-codes');
  for(const code of given)out.add('context-unavailable',code);
 }
 return finish({strategyId:'session-intraday-v1',key:result.key||null,phase,rawSourceState:source.state,rawResultState:result.state,side:source.side==='CALL'||source.side==='PUT'?source.side:null,reasons:out.rows});
}
/** Stable event identity is included; tick clocks, ratios, strengths and free text are not. */
export function chartSignalDiagnosticsSemanticKey(report:Pick<ChartSignalDiagnosticsReport,'strategies'|'coverage'>):string {
 return JSON.stringify([
  CHART_SIGNAL_DIAGNOSTICS_VERSION,
  [...report.strategies].sort((a,b)=>a.strategyId.localeCompare(b.strategyId)).map(row=>[row.strategyId,row.key,row.phase,row.rawSourceState,row.rawResultState,row.side,[...row.reasons].map(reason=>[reason.category,reason.code]).sort((a,b)=>a[0].localeCompare(b[0])||a[1].localeCompare(b[1]))]),
  sorted(report.coverage.reasonCodes),
  [...report.coverage.rows].sort((a,b)=>a.kind.localeCompare(b.kind)).map(row=>[row.kind,row.status,row.complete,sorted(row.reasonCodes)])
 ]);
}
/** Reads already computed results. It never evaluates a strategy or changes a gate. */
export function buildChartSignalDiagnostics(input:ChartSignalDiagnosticsInput):Readonly<ChartSignalDiagnosticsReport> {
 const coverage=structuredClone(input.coverage),strategies=[importantDiagnostic(input),intradayDiagnostic(input)];
 return freeze({version:CHART_SIGNAL_DIAGNOSTICS_VERSION,symbol:coverage.symbol,sessionKey:coverage.sessionKey,evaluatedMs:coverage.evaluatedMs,publishedMs:coverage.publishedMs,connectionStartedMs:coverage.connectionStartedMs,coverage,strategies,semanticKey:chartSignalDiagnosticsSemanticKey({coverage,strategies}),confirmationOrigin:'existing-source-only' as const,entryAuthorized:false as const,executionEnabled:false as const,probability:null,admissionModified:false as const,signalsCreated:false as const});
}

const record=(value:unknown):value is Record<string,unknown>=>!!value&&typeof value==='object'&&!Array.isArray(value);
const text=(value:unknown,max=2048):value is string=>typeof value==='string'&&value.length<=max;
const quantity=(value:unknown)=>value===null||typeof value==='number'&&Number.isFinite(value)&&value>=0;
const onlyKeys=(value:Record<string,unknown>,allowed:readonly string[])=>Object.keys(value).every(key=>allowed.includes(key));
const count=(value:unknown)=>value===null||typeof value==='number'&&Number.isSafeInteger(value)&&value>=0;
function feedEnvelope(value:unknown):boolean {
 if(!record(value)||!onlyKeys(value,['stocks','options','optionCapture','flow','cvd']))return false;
 for(const key of ['stocks','options']){
  const row=value[key];if(row===null)continue;
  if(!record(row)||row.scope!=='connection-feed-not-asset-window'||!(row.phase===null||text(row.phase,128))||![row.events,row.malformed,row.reconnects].every(count)||![row.lastEventMs,row.lastReceivedMs].every(maybeClock)||!onlyKeys(row,['scope','phase','events','malformed','reconnects','lastEventMs','lastReceivedMs']))return false;
 }
 const option=value.optionCapture;
 if(option!==null&&(!record(option)||!(option.method===null||text(option.method,256))||![option.requestedChannels,option.acknowledgedChannels,option.contractsWithEvents].every(count)||![option.lastContractEventMs,option.connectionStartedMs].every(maybeClock)||option.receptionPercent!==null||!codes(option.reasonCodes)||!onlyKeys(option,['method','requestedChannels','acknowledgedChannels','contractsWithEvents','lastContractEventMs','connectionStartedMs','receptionPercent','reasonCodes'])))return false;
 const flow=value.flow;
 if(flow!==null&&(!record(flow)||flow.scope!=='flow-engine-cumulative-not-window'||![flow.capacityDrops,flow.rejectedTrades].every(count)||!onlyKeys(flow,['scope','capacityDrops','rejectedTrades'])))return false;
 const cvd=value.cvd;
 return cvd===null||record(cvd)&&cvd.scope==='observed-segment-not-window'&&[cvd.rejected,cvd.regularGaps].every(count)&&maybeClock(cvd.regularSinceMs)&&onlyKeys(cvd,['scope','rejected','regularGaps','regularSinceMs']);
}
/** Bounded JSON envelope only; source association and quantitative acceptance belong to the caller. */
function jsonBounded(value:unknown,depth=0,budget={nodes:0}):boolean {
 if(depth>12||++budget.nodes>4096)return false;
 if(value===null||typeof value==='boolean')return true;
 if(typeof value==='number')return Number.isFinite(value);
 if(typeof value==='string')return value.length<=16384;
 if(Array.isArray(value))return value.length<=256&&value.every(child=>jsonBounded(child,depth+1,budget));
 return record(value)&&Object.keys(value).length<=64&&Object.entries(value).every(([key,child])=>key.length<=96&&jsonBounded(child,depth+1,budget));
}
function coverageEnvelope(value:unknown):value is ChartSignalCoverageReport {
 if(!record(value)||value.version!=='chart-signal-coverage-v1'||!text(value.symbol,32)||!value.symbol||!(value.sessionKey===null||text(value.sessionKey,512))||![value.evaluatedMs,value.publishedMs,value.connectionStartedMs].every(maybeClock)||!codes(value.reasonCodes)||!Array.isArray(value.rows)||value.rows.length!==KINDS.length)return false;
 const kinds=new Set();
 for(const row of value.rows){
  if(!record(row)||!KINDS.includes(row.kind as ChartSignalCoverageKind)||kinds.has(row.kind)||!['shares','contracts','USD','events','minutes'].includes(row.unit as string)||!text(row.sourcePath)||!['available','partial','unavailable','invalid'].includes(row.status as string)||![row.numerator,row.denominator,row.percent].every(quantity)||typeof row.percent==='number'&&row.percent>100||!(row.complete===null||typeof row.complete==='boolean')||!codes(row.reasonCodes)||!record(row.window)||!record(row.freshness))return false;
  if(![row.window.startMs,row.window.endMs].every(maybeClock)||!quantity(row.window.durationMs)||!['tick-time','calendar-closed-minutes','source-recent-five','flow-asof-minus-duration','feed-connection','unavailable'].includes(row.window.boundsBasis as string))return false;
  if(![row.freshness.sourceAtMs,row.freshness.lastEventMs].every(maybeClock)||![row.freshness.sourceAgeMs,row.freshness.publishedAgeMs,row.freshness.publicationLagMs,row.freshness.eventAgeMs].every(quantity)||row.amountVerifiedUsd!==undefined&&!quantity(row.amountVerifiedUsd))return false;
  kinds.add(row.kind);
 }
 if(!feedEnvelope(value.feedDiagnostics)||!record(value.existingConsumer)||!onlyKeys(value.existingConsumer,['flow','cvd']))return false;
 for(const key of ['flow','cvd']){
  const pressure=value.existingConsumer[key];if(pressure===null)continue;
  if(!record(pressure)||!(pressure.signal===null||typeof pressure.signal==='number'&&Number.isFinite(pressure.signal)&&Math.abs(pressure.signal)<=1)||!quantity(pressure.coverage)||typeof pressure.coverage==='number'&&pressure.coverage>100||!maybeClock(pressure.at)||!text(pressure.reason))return false;
 }
 return true;
}
export function validChartSignalDiagnostics(value:unknown):value is Readonly<ChartSignalDiagnosticsReport> {
 if(!record(value)||!jsonBounded(value)||value.version!==CHART_SIGNAL_DIAGNOSTICS_VERSION||!text(value.symbol,32)||!value.symbol||!(value.sessionKey===null||text(value.sessionKey,512))||![value.evaluatedMs,value.publishedMs,value.connectionStartedMs].every(maybeClock)||value.confirmationOrigin!=='existing-source-only'||value.entryAuthorized!==false||value.executionEnabled!==false||value.probability!==null||value.admissionModified!==false||value.signalsCreated!==false||!text(value.semanticKey,16384)||!coverageEnvelope(value.coverage)||!Array.isArray(value.strategies)||value.strategies.length!==2)return false;
 if(!onlyKeys(value,['version','symbol','sessionKey','evaluatedMs','publishedMs','connectionStartedMs','coverage','strategies','semanticKey','confirmationOrigin','entryAuthorized','executionEnabled','probability','admissionModified','signalsCreated']))return false;
 const coverage=value.coverage;if(value.symbol!==coverage.symbol||value.sessionKey!==coverage.sessionKey||value.evaluatedMs!==coverage.evaluatedMs||value.publishedMs!==coverage.publishedMs||value.connectionStartedMs!==coverage.connectionStartedMs)return false;
 const seen=new Set();
 for(const row of value.strategies){
  if(!record(row)||!['important-level-route-v1','session-intraday-v1'].includes(row.strategyId as string)||seen.has(row.strategyId)||!(row.key===null||text(row.key,2048)&&row.key.length>0)||!PHASES.includes(row.phase as string)||!SOURCE_STATES.includes(row.rawResultState as string)||!(row.side===null||row.side==='CALL'||row.side==='PUT')||!codes(row.reasonCodes)||!Array.isArray(row.reasons)||row.reasons.length>64)return false;
  if(row.strategyId==='important-level-route-v1'?(row.rawSourceState!==row.rawResultState||['confirmed','blocked'].includes(row.phase as string)):!INTRADAY_STATES.includes(row.rawSourceState as string))return false;
  if(row.rawResultState==='reached'?row.phase!=='reached':row.rawResultState==='invalidated'?!['invalidated','expired'].includes(row.phase as string):row.strategyId==='important-level-route-v1'?row.phase!==(['recovery','test','sustained'].includes(row.rawResultState as string)?'candidate':'waiting'):row.phase!==row.rawSourceState)return false;
  const pairs=new Set();
  for(const reason of row.reasons){if(!record(reason)||!CATEGORIES.includes(reason.category as string)||!machineCode(reason.code)||pairs.has(String(reason.category)+':'+reason.code))return false;pairs.add(String(reason.category)+':'+reason.code);if(reason.category==='actual-veto'&&(row.strategyId!=='session-intraday-v1'||row.rawSourceState!=='blocked'))return false;}
  if(JSON.stringify(sorted(row.reasonCodes))!==JSON.stringify(sorted(row.reasons.map((reason:ChartSignalDiagnosticReason)=>reason.code))))return false;
  seen.add(row.strategyId);
 }
 return value.semanticKey===chartSignalDiagnosticsSemanticKey(value as unknown as ChartSignalDiagnosticsReport);
}
/** Missing optional extension is valid for all existing v1 histories and absent reads. */
export function validOptionalChartSignalDiagnostics(value:unknown):value is Readonly<ChartSignalDiagnosticsReport>|null|undefined {
 return value===undefined||value===null||validChartSignalDiagnostics(value);
}
