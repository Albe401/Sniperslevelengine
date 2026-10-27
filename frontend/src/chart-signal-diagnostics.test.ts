import {expect,it} from 'vitest';
import {buildChartSignalDiagnostics,chartSignalDiagnosticsSemanticKey,validChartSignalDiagnostics,validOptionalChartSignalDiagnostics,type ChartSignalDiagnosticsInput} from './chart-signal-diagnostics';
import type {ChartSignalCoverageReport,ChartSignalCoverageKind} from './chart-signal-coverage';
import type {RouteResult} from './route-engine';
import type {RouteStructure} from './route-level-context';
import type {RouteCvdSupport} from './route-cvd';

const now=1793023200000;
const kinds:ChartSignalCoverageKind[]=['price-clock','closed-minute-continuity','stock-share-classification','option-contract-classification','option-premium-classification','option-premium-availability','stock-reception','option-reception'];
function coverage():ChartSignalCoverageReport {
 return {version:'chart-signal-coverage-v1',symbol:'QQQ',sessionKey:'QQQ:2026-10-26:1793019600000',evaluatedMs:now,publishedMs:now-50,connectionStartedMs:1793019600000,reasonCodes:[],rows:kinds.map(kind=>({kind,unit:kind.includes('premium')?'USD':kind.includes('contract')?'contracts':kind.includes('reception')?'events':kind==='closed-minute-continuity'?'minutes':'shares',sourcePath:'fixture.'+kind,numerator:80,denominator:100,percent:80,complete:true,status:'available',window:{startMs:now-300050,endMs:now-50,durationMs:300000,boundsBasis:'source-recent-five'},freshness:{sourceAtMs:now-50,sourceAgeMs:50,publishedAgeMs:50,publicationLagMs:0,lastEventMs:now-100,eventAgeMs:100},reasonCodes:[]})),feedDiagnostics:{stocks:null,options:null,optionCapture:null,flow:null,cvd:null},existingConsumer:{flow:{signal:.3,coverage:80,at:now-50,reason:'fixture'},cvd:{signal:.4,coverage:80,at:now-50,reason:'fixture'}}};
}
const route=(state:RouteResult['state']='sustained'):RouteResult=>({state,direction:1,level:100,target:102,stop:99.97,reason:'Human text is not parsed',at:now,key:'QQQ:episode-1',deadlineMs:now+600000});
type MutableInput=Omit<ChartSignalDiagnosticsInput,'coverage'|'important'>&{coverage:ChartSignalCoverageReport;important:{result:RouteResult;assessmentQuality:ChartSignalDiagnosticsInput['important']['assessmentQuality'];cvd:RouteCvdSupport|null;structure:RouteStructure|null}};
function input():MutableInput {
 const structure:RouteStructure={quality:'supported',requiresAdditionalTest:false,confirmationBlocked:false,backing:null,anchor:null,ahead:[],levels:[],reason:'Descriptive text is not parsed'};
 return {coverage:coverage(),important:{result:route(),assessmentQuality:'aligned',cvd:{status:'aligned',label:'Acompaña',detail:'fixture',at:now-50,coverage:80,observations:2},structure},intraday:{state:'confirmed',result:route(),side:'CALL',blockCodes:[]}};
}
const own=(report:ReturnType<typeof buildChartSignalDiagnostics>,strategy='session-intraday-v1')=>report.strategies.find(row=>row.strategyId===strategy)!;

it('copies source clocks and freezes detached diagnostics without mutating existing engines or coverage',()=>{
 const source=input(),before=JSON.stringify(source),report=buildChartSignalDiagnostics(source);
 expect(report).toMatchObject({version:'chart-signal-diagnostics-v1',symbol:'QQQ',sessionKey:source.coverage.sessionKey,evaluatedMs:now,publishedMs:now-50,connectionStartedMs:1793019600000,entryAuthorized:false,executionEnabled:false,probability:null,admissionModified:false,signalsCreated:false});
 expect(JSON.stringify(source)).toBe(before);expect(Object.isFrozen(source.coverage)).toBe(false);
 expect(Object.isFrozen(report)).toBe(true);expect(Object.isFrozen(report.coverage.rows[0].freshness)).toBe(true);
 expect(Object.isFrozen(report.strategies[0].reasons)).toBe(true);expect(report.coverage).not.toBe(source.coverage);
 source.important.result.reason='Later text';source.coverage.existingConsumer.flow={...source.coverage.existingConsumer.flow!,reason:'Later pressure text'};
 expect(report.coverage.existingConsumer.flow!.reason).toBe('fixture');
});

it.each(['aligned','partial','opposed'] as const)('keeps legacy sustained observation a candidate when assessment is %s',quality=>{
 const source=input();source.important.assessmentQuality=quality;
 const row=own(buildChartSignalDiagnostics(source),'important-level-route-v1');
 expect(row).toMatchObject({phase:'candidate',rawSourceState:'sustained',rawResultState:'sustained',key:'QQQ:episode-1'});
 expect(row.reasons.filter(reason=>reason.category==='actual-veto')).toEqual([]);
 expect(row.reasonCodes).toContain('legacy-candidate-no-qualified-admission');
});

it('keeps source confirmation when optional options/CVD context is absent instead of inventing a veto',()=>{
 const source=input();source.coverage.existingConsumer={flow:null,cvd:null};
 source.coverage.rows=source.coverage.rows.map(row=>row.kind.startsWith('option-')?{...row,status:'unavailable',complete:null,numerator:null,denominator:null,percent:null,reasonCodes:['missing-option-data']}:row);
 const report=buildChartSignalDiagnostics(source),intraday=own(report),important=own(report,'important-level-route-v1');
 expect(intraday.phase).toBe('confirmed');expect(intraday.reasons.filter(reason=>reason.category==='actual-veto')).toEqual([]);
 expect(intraday.reasons).toContainEqual({category:'context-unavailable',code:'option-reception-unavailable'});
 expect(important.reasonCodes).toContain('option-flow-context-unavailable');expect(important.reasonCodes).toContain('stock-cvd-context-unavailable');
 expect(source.intraday.state).toBe('confirmed');expect(report.admissionModified).toBe(false);
});

it('classifies only the existing blocked source codes as actual vetoes',()=>{
 const source=input();source.intraday.state='blocked';source.intraday.blockCodes=['cvd-opposed','weak-level-backing','cvd-opposed'];
 source.coverage.rows=source.coverage.rows.map(row=>row.kind==='option-contract-classification'?{...row,status:'partial',complete:false,reasonCodes:['incomplete-window']}:row);
 const row=own(buildChartSignalDiagnostics(source));
 expect(row.phase).toBe('blocked');expect(row.reasons.filter(reason=>reason.category==='actual-veto')).toEqual([{category:'actual-veto',code:'cvd-opposed'},{category:'actual-veto',code:'weak-level-backing'}]);
 expect(row.reasons).toContainEqual({category:'context-unavailable',code:'option-contract-classification-partial'});
});

it('explains legacy blocked reads without guessing a veto from Spanish free text',()=>{
 const source=input();source.intraday.state='blocked';source.intraday.blockCodes=undefined;source.intraday.result.reason='CVD en contra; fuerza insuficiente; price-direction-opposed';
 const row=own(buildChartSignalDiagnostics(source));
 expect(row.phase).toBe('blocked');expect(row.reasons.filter(reason=>reason.category==='actual-veto')).toEqual([]);
 expect(row.reasonCodes).toContain('source-block-codes-unavailable');expect(row.reasonCodes).not.toContain('price-direction-opposed');
});

it('separates pending sequence and unavailable context for source candidate/waiting states',()=>{
 const source=input();source.intraday.state='candidate';source.intraday.blockCodes=['missing-geometry'];source.intraday.result=route('test');
 expect(own(buildChartSignalDiagnostics(source)).reasons).toContainEqual({category:'dependent-sequence',code:'missing-geometry'});
 source.intraday.state='waiting';source.intraday.blockCodes=['no-candidate-yet','price-unready','history-revised'];
 const row=own(buildChartSignalDiagnostics(source));expect(row.phase).toBe('waiting');
 expect(row.reasons).toContainEqual({category:'dependent-sequence',code:'no-candidate-yet'});
 expect(row.reasons).toContainEqual({category:'dependent-sequence',code:'history-revised'});
 expect(row.reasons).toContainEqual({category:'context-unavailable',code:'price-unready'});
});

it.each(['recovery','test'] as const)('retains important %s dependencies without changing a quality veto',state=>{
 const source=input();source.important.result=route(state);source.important.structure={...source.important.structure!,quality:'limited',requiresAdditionalTest:true,confirmationBlocked:true};
 const row=own(buildChartSignalDiagnostics(source),'important-level-route-v1');expect(row.phase).toBe('candidate');
 expect(row.reasons.filter(reason=>reason.category==='actual-veto')).toEqual([]);
 if(state==='test')expect(row.reasons).toContainEqual({category:'dependent-sequence',code:'additional-price-continuation-pending'});
});

it('uses explicit expiry metadata and source state instead of parsing the reason',()=>{
 const source=input();source.intraday.state='invalidated';source.intraday.result={...route('invalidated'),terminalCause:'expired'};source.intraday.blockCodes=['sequence-expired'];
 expect(own(buildChartSignalDiagnostics(source))).toMatchObject({phase:'expired',rawSourceState:'invalidated',rawResultState:'invalidated'});
 source.intraday.result=route('invalidated');source.intraday.result.reason='La secuencia excedió diez minutos';source.intraday.blockCodes=[];
 expect(own(buildChartSignalDiagnostics(source)).phase).toBe('invalidated');
});

it('does not change semantic key for evaluator/tick clocks, ratios or free text',()=>{
 const source=input(),before=buildChartSignalDiagnostics(source).semanticKey;
 source.coverage.evaluatedMs=now+1000;source.coverage.publishedMs=now+950;source.coverage.existingConsumer.flow={...source.coverage.existingConsumer.flow!,signal:.9};
 source.coverage.rows=source.coverage.rows.map(row=>({...row,numerator:95,percent:95,freshness:{...row.freshness,sourceAgeMs:1000}}));
 source.important.result.at+=1000;source.important.result.reason='New language';source.important.cvd={...source.important.cvd!,coverage:95,observations:20};
 expect(buildChartSignalDiagnostics(source).semanticKey).toBe(before);
});

it('records a new episode with the same phase and codes',()=>{
 const source=input(),before=buildChartSignalDiagnostics(source).semanticKey;source.intraday.result={...source.intraday.result,key:'QQQ:episode-2'};
 expect(buildChartSignalDiagnostics(source).semanticKey).not.toBe(before);
});

it.each(['status','complete','codes'] as const)('changes semantic key when coverage %s changes',field=>{
 const source=input(),before=buildChartSignalDiagnostics(source).semanticKey;
 source.coverage.rows=source.coverage.rows.map((row,i)=>i?row:{...row,...(field==='status'?{status:'partial' as const}:field==='complete'?{complete:false}:{reasonCodes:['source-gap']})});
 expect(buildChartSignalDiagnostics(source).semanticKey).not.toBe(before);
});

it('deduplicates and sorts reasons so input order does not create journal churn',()=>{
 const source=input();source.intraday.state='blocked';source.intraday.blockCodes=['cvd-opposed','weak-level-backing'];source.coverage.reasonCodes=['stale-publication','outside-rth'];
 const before=buildChartSignalDiagnostics(source);source.intraday.blockCodes.reverse();source.coverage.reasonCodes=[...source.coverage.reasonCodes].reverse();source.coverage.rows=[...source.coverage.rows].reverse();
 expect(buildChartSignalDiagnostics(source).semanticKey).toBe(before.semanticKey);
 expect(chartSignalDiagnosticsSemanticKey(before)).toBe(before.semanticKey);
});

it('accepts absent optional legacy fields and JSON recovery of a valid envelope',()=>{
 expect(validOptionalChartSignalDiagnostics(undefined)).toBe(true);expect(validOptionalChartSignalDiagnostics(null)).toBe(true);
 const report=buildChartSignalDiagnostics(input());expect(validChartSignalDiagnostics(report)).toBe(true);
 expect(validChartSignalDiagnostics(JSON.parse(JSON.stringify(report)))).toBe(true);
});

it.each(['version','execution','probability','association','reason-code','duplicate-strategy','nan','semantic-key','unknown-fields','invented-confirmation'] as const)('rejects a malformed %s envelope',kind=>{
 const value=structuredClone(buildChartSignalDiagnostics(input())) as any;
 if(kind==='version')value.version='other';if(kind==='execution')value.executionEnabled=true;if(kind==='probability')value.probability=.9;
 if(kind==='association')value.publishedMs++;if(kind==='reason-code')value.strategies[0].reasonCodes.push('Spanish text, not a code');
 if(kind==='duplicate-strategy')value.strategies[1]=structuredClone(value.strategies[0]);if(kind==='nan')value.coverage.rows[0].percent=NaN;if(kind==='semantic-key')value.semanticKey+='changed';
 if(kind==='unknown-fields')value.orders=[];
 if(kind==='invented-confirmation'){value.strategies[0].phase='confirmed';value.semanticKey=chartSignalDiagnosticsSemanticKey(value);}
 expect(validOptionalChartSignalDiagnostics(value)).toBe(false);
});

it('preserves invalid-clock diagnostics as evidence without enforcing a new admission gate',()=>{
 const source=input();source.coverage.evaluatedMs=null;source.coverage.reasonCodes=['invalid-evaluation-clock'];source.coverage.rows=source.coverage.rows.map(row=>({...row,status:'invalid',reasonCodes:['invalid-evaluation-clock']}));
 const report=buildChartSignalDiagnostics(source);expect(report.evaluatedMs).toBeNull();expect(own(report).phase).toBe('confirmed');expect(validChartSignalDiagnostics(report)).toBe(true);
 source.coverage.evaluatedMs=now;source.coverage.publishedMs=now+5000;source.coverage.reasonCodes=['future-publication'];
 const future=buildChartSignalDiagnostics(source);expect(future.publishedMs).toBe(now+5000);expect(validChartSignalDiagnostics(future)).toBe(true);
});
