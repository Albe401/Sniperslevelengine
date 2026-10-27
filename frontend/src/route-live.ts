import {readChartSignalCoverage} from './chart-signal-coverage';
import {buildChartSignalDiagnostics,type ChartSignalDiagnosticsReport} from './chart-signal-diagnostics';
import {publicationCandleClock} from './publication-candle-clock';
import {SessionIntradayRoute,type IntradayRouteRead} from './session-intraday-route';
import {routeLevelDestinations} from './route-level-destinations';
import {EarlyRouteEvaluation} from './early-route-evaluation';
import {routeEmaContext} from './route-ema-context';
import {QuantRouteMonitor,quantRouteFrame} from './quant-route-live';
import {OptionBaselineArchive,type BaselinePacket} from './option-baseline-archive';
import {persistOptionBaseline,optionResearchNote,pollOptionResearch} from './option-research-live';
import {directionalRoute,type DirectionalRoute} from './directional-route';
import {intradayAtr,type AtrRead} from './intraday-atr';
import {readMarketEvidence,type MarketEvidence} from './market-evidence';
import {MarketSignalTracker,type MarketSignal} from './market-signals';
import {RouteCvdTracker} from './route-cvd';
import {assessRoute} from './route-assessment';
import {routeProbabilityCollector,probabilityEndpoint} from './route-probability';
import {routeProbabilityReading,visibleRouteReading,type ProbabilityReading} from './route-probability-reading';
import type {LiveState} from './live';
import {currentSessionCandles} from './candle-series';
import {readPriceNow} from './price-now';
import {readStrikeAbsorption} from './strike-absorption';
import {RouteEngine,type RouteLevel,type RouteEvidence,type RouteResult} from './route-engine';
import {mapRouteLevels,assessRouteStructure} from './route-level-context';
let baselineStorage:Storage|undefined;try{baselineStorage=localStorage;}catch{/* Memory archive remains exportable. */}
const formulaBaseline=new OptionBaselineArchive(baselineStorage);
/** Independent route state for the selected view and autonomous QQQ control. */
export function createRouteReader(background=false){
const engine=new RouteEngine();
const sessionIntraday=new SessionIntradayRoute();
const quantRoute=new QuantRouteMonitor(!background);

const earlyEvaluation=new EarlyRouteEvaluation(!background);
const cvdTracker=new RouteCvdTracker();
const signalTracker=new MarketSignalTracker();
let lastObserved=0;
let identity='',known=new Map<string,RouteLevel>(),lastJournal='',storageFailed=false;
let chartReading:{symbol:string;at:number;result:RouteResult;cvd:string;market:MarketEvidence;signals:MarketSignal[];probability:ProbabilityReading;directional:DirectionalRoute;intraday:IntradayRouteRead;quant:ReturnType<QuantRouteMonitor['read']>;evaluation:ReturnType<EarlyRouteEvaluation['read']>;emaContext:ReturnType<typeof routeEmaContext>}|undefined;
let signalDiagnostics:Readonly<ChartSignalDiagnosticsReport>|undefined;
let signalDiagnosticsFailedAt:{symbol:string;at:number}|undefined;
/** Read-only view of the route already evaluated by the panel; never advances the engine. */
function routeChartReading(symbol:string,now=Date.now()){return chartReading?.symbol===symbol&&chartReading.at<=now&&now-chartReading.at<=3000?{...chartReading,result:{...chartReading.result},quant:{...chartReading.quant,probabilities:chartReading.quant.probabilities?.slice()??null,signals:chartReading.quant.signals.map(v=>({...v}))},intraday:{...chartReading.intraday,blockCodes:chartReading.intraday.blockCodes?.slice(),atr:chartReading.intraday.atr?{...chartReading.intraday.atr}:undefined,result:{...chartReading.intraday.result},signals:chartReading.intraday.signals.map(v=>({...v}))},signals:chartReading.signals.map(s=>({...s})),directional:{...chartReading.directional,checks:{...chartReading.directional.checks}}}:undefined;}
/** Read-only diagnostic at its render clock; web exports remain snapshot references, never live signals. */
function routeSignalDiagnosticsSnapshot(symbol:string,now=Date.now()){if(signalDiagnosticsFailedAt?.symbol===symbol&&signalDiagnosticsFailedAt.at===now)return null;const d=signalDiagnostics;return Number.isSafeInteger(now)&&d?.symbol===symbol&&d.evaluatedMs!==null&&d.evaluatedMs<=now&&now-d.evaluatedMs<=3000?d:undefined;}
const journalKey='snipers.route-observation.v1';
const routeEtFormatter=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
const et=(n:number)=>routeEtFormatter.format(n);
const px=(n:number|null)=>n==null?'Pendiente':`$${n.toFixed(2)}`;
let journal:unknown[]=[];
try{const saved=JSON.parse(localStorage.getItem(journalKey)??'[]');if(Array.isArray(saved))journal=saved.slice(-1000);}catch{/* Missing storage never fabricates evidence. */}
function record(payload:unknown,key:string){
 if(key===lastJournal)return;lastJournal=key;journal.push(payload);journal=journal.slice(-1000);
 try{let data=JSON.stringify(journal);while(data.length>1500000&&journal.length>1){journal.shift();data=JSON.stringify(journal);}localStorage.setItem(journalKey,data);storageFailed=false;}catch{storageFailed=true;}
}
function routeJournal(){return JSON.stringify({schema:'route-observation-v1',experimental:true,executionEnabled:false,formulaBaseline:formulaBaseline.export(),probabilityEndpoint,probabilitySamples:routeProbabilityCollector.snapshot(),signalDiagnostics:signalDiagnostics??null,signalDiagnosticsStatus:signalDiagnosticsFailedAt?'failed':signalDiagnostics?'available':'missing',signalDiagnosticsMode:'snapshot-reference-only',signalDiagnosticsLive:false,quantSamples:quantRoute.engine.snapshot(),earlyRouteSamples:earlyEvaluation.engine.snapshot(),arrivalSamples:earlyEvaluation.arrivals.samples(),arrivalObservations:earlyEvaluation.arrivals.snapshot(),records:journal},null,2);}
function bindRouteJournal(){
 const button=document.querySelector<HTMLButtonElement>('[data-route-export]');
 if(button)button.onclick=()=>{
  const url=URL.createObjectURL(new Blob([routeJournal()],{type:'application/json'}));
  const link=document.createElement('a');link.href=url;link.download=`ruta-precio-${et(Date.now())}.json`;link.click();
  setTimeout(()=>URL.revokeObjectURL(url),30000);
 };
}
function readRouteLive(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now(),atr?:AtrRead) {
 pollOptionResearch(now);
 const input=s?.ticks&&s.stocks&&s.metrics&&s.flow?s:undefined;
 const price=readPriceNow(input,symbol,active,now),rows=readStrikeAbsorption(s,symbol,active,now).rows;
 const market=readMarketEvidence(s,symbol,active,now);
 const magnitude=atr??(s?.stocks&&s.metrics&&s.chartHistory?intradayAtr(s,symbol,active,now):undefined);
 const intraday=sessionIntraday.read(s,symbol,active,market,now,magnitude);
 const mapLevels=market.levels;
 const expiry=mapLevels[0]?.expiration??rows[0]?.expiration??'',next=`${symbol}:${et(now)}:${s?.startedMs??0}:${expiry}`;
 if(identity!==next||now<lastObserved){engine.reset('Activo, sesión o vencimiento nuevos');signalTracker.reset();cvdTracker.reset();known.clear();identity=next;}
 lastObserved=now;
 // Levels become usable only after actually being seen, never retrofitted into old bars.
 const levels:RouteLevel[]=mapLevels.map(row=>{
  const key=`${row.name}:${row.price}:${row.expiration}`,existing=known.get(key);
  const level={...row,observed:existing?.observed??now};known.set(key,level);return level;
 });
 for(const key of known.keys())if(!mapLevels.some(r=>`${r.name}:${r.price}:${r.expiration}`===key))known.delete(key);
 const optionAt=s?.options.lastEventMs;
 const flowFresh=active&&s?.symbol===symbol&&s.options.phase==='streaming'&&optionAt!=null&&optionAt<=now&&now-optionAt<=15000;
 const w=flowFresh?s?.flow.intraday_windows?.find(w=>w.minutes===5):undefined;
 const flowLabel=({'BULLISH FLOW':'predominio alcista','BEARISH FLOW':'predominio bajista','BALANCED FLOW':'mixta'} as Record<string,string>)[w?.direction??'']??'sin predominio verificable';
 const flowNote=!w||w.event_count<=0?'Prima: esperando ejecuciones':!w.premium_complete?'Prima: muestra parcial':`Prima: ${flowLabel} · clasificación inferida`;
 const deltaNote=rows.some(r=>r.at!=null&&(r.call!=null||r.put!=null))?'Delta: sensibilidad del snapshot; no confirma dirección':'Delta: pendiente';
 const blocks=s?.offExchange?.analytics?.blocks;
 const blockNote=blocks?.symbol===symbol&&blocks.date===et(now)?'Bloques: contexto separado de premarket y sesión':'Bloques: sin evidencia del día';
 const evidence:RouteEvidence={identity:next,now,levels,options:flowNote,delta:deltaNote,blocks:blockNote};
 const bars=currentSessionCandles(s?.chartHistory,1,publicationCandleClock(s?.publishedMs,now)).filter(b=>b.complete&&b.sessionDate===et(now));
 const last=bars.at(-1);
 if(!price.ready||!last||expiry!==et(now)){engine.reset(expiry&&expiry!==et(now)?'Niveles de otro vencimiento: prueba intradía pendiente':'Esperando precio, conexión, velas y niveles de la sesión');}
 else{
  // Canonical OHLCV only, avoid VWAP revisions masquerading as corrected candles.
  const clean=bars.map(({timestampMs,open,high,low,close,volume})=>({timestampMs,open,high,low,close,volume}));
  const tail=clean.at(-1)!;
  evidence.structure=assessRouteStructure(levels.filter(l=>l.observed<=tail.timestampMs),tail.close,engine.candidate?.direction??0,engine.candidate?.anchor.price??null,now);
  if(engine.bars.some(old=>{const match=clean.find(b=>b.timestampMs===old.timestampMs);return match&&JSON.stringify(match)!==JSON.stringify(old);})){engine.reset('Historial corregido: nueva prueba requerida');signalTracker.reset();}
  if(engine.candidate&&!levels.some(l=>l.name===engine.candidate!.anchor.name&&l.price===engine.candidate!.anchor.price))engine.reset('Referencia cambió: nueva prueba requerida');
  if(!engine.bars.length){
   let contiguous=clean.slice(-20);for(let i=contiguous.length-1;i>0;i--)if(contiguous[i].timestampMs-contiguous[i-1].timestampMs!==60000){contiguous=contiguous.slice(i);break;}
   engine.warm(contiguous,next);
  }else engine.feed(tail,evidence);
 }
 const result={...engine.result};
 if(!background)routeProbabilityCollector.observe(s,market,result,bars,active,now);
 const structure=assessRouteStructure(levels,price.spot??last?.close??0,engine.candidate?.direction??result.direction,result.level,now);
 evidence.structure=structure;
 const cvd=cvdTracker.read(s,result,symbol,active,last?.timestampMs,now);
 const probability=visibleRouteReading(routeProbabilityReading(symbol,now,result.key),market.score);
 const signals=signalTracker.read(market,result,last?.timestampMs,active&&price.ready&&levels.length>0,probability);
 const directional=directionalRoute(s,symbol,active,market,result,structure,now,magnitude);
 const destinations=active?routeLevelDestinations(market,now,magnitude):[];
 const evaluation=earlyEvaluation.read(s,market,result,magnitude,bars,active,now),emaContext=routeEmaContext(active?s:undefined,now);
 const quantFrame=quantRouteFrame(s,market,result,intraday,magnitude,active,now);
 const quant=quantRoute.read(quantFrame,bars);
 if(background&&active&&s?.symbol===symbol&&s.metrics?.session===et(now)&&s.metrics?.phase==='Sesión RTH'&&Number.isSafeInteger(s.publishedMs)){
  // Only the two-minute capture period following a five-minute close needs baseline packets.
  const recorded=Date.now();
  if(recorded%300000<=120000&&recorded>=now&&recorded-now<=3000){const packet:BaselinePacket={version:'option-formula-baseline-v1',source:'desktop-live',symbol,session:et(recorded),observedMs:recorded,publishedMs:s.publishedMs!,connection:s.startedMs??0,
   confluence:{version:'direction-confluence-40-40-20-v1',call:market.score.call,put:market.score.put,side:market.score.side,components:market.score.components},
   quant:{version:'quant-route-close-first-10m-v1',horizonMinutes:10,state:quant.state,at:quant.at,probabilities:quant.probabilities,sampleId:quant.sampleId,side:quant.side,upper:quant.upper,lower:quant.lower}};formulaBaseline.observe(packet);persistOptionBaseline(packet);}
 }
 const assessment=assessRoute(result,market,now);
 // Passive diagnostics must not suppress an existing source strategy or snapshot rendering.
 signalDiagnostics=undefined;signalDiagnosticsFailedAt=undefined;
 try{const coverage=readChartSignalCoverage(s,market,symbol,active,now,{closedMinuteBars:bars});signalDiagnostics=buildChartSignalDiagnostics({coverage,important:{result,assessmentQuality:assessment.quality,cvd,structure},intraday});}catch{signalDiagnosticsFailedAt={symbol,at:now};}
 chartReading={symbol,at:now,result:{...result},cvd:cvd.label,market,signals,probability,directional,intraday,quant,evaluation,emaContext};
 if(!background&&active&&s?.symbol===symbol){
  const key=JSON.stringify([next,last?.timestampMs,result.state,result.key,result.reason,structure.quality,structure.backing?.grade,structure.anchor?.grade,cvd.status,cvd.observations,intraday.state,intraday.result.key,intraday.reason,directional.state,directional.checks]);
  record({capturedMs:now,symbol,session:et(now),expiration:expiry,bar:last?{timestampMs:last.timestampMs,open:last.open,high:last.high,low:last.low,close:last.close,volume:last.volume}:null,evidence,result,
   quality:{priceReady:price.ready,publishedMs:s.publishedMs,optionAt,executionConditionsVerified:false,combinedSignalValidated:false},
   marketEvidence:market,destinations,evaluation,emaContext,directional,intraday,quant,quantFrame,probability,signals,cvdSupport:cvd,cvdInput:s.cvd??null,flow:w??null,strikes:rows,blocks:blocks??null},key+JSON.stringify(destinations.map(d=>[d.target,d.strength,d.provisional]))+JSON.stringify([quantFrame.ready,quantFrame.reason,quant.samples]));
 }
 return {result,structure,cvd,market,signals,assessment,probability,directional,intraday,quant,conclusion:assessment.label,reason:result.reason,target:result.target==null?null:`Siguiente referencia ${px(result.target)} · llegada no garantizada`,
  invalidation:result.stop==null?null:`Cierre ${result.level!=null&&result.stop>result.level?'sobre':'bajo'} ${px(result.stop)} invalida la prueba`,
  note:`Prueba en vivo · sin ejecución · ${flowNote} · ${deltaNote} · ${blockNote} · ${market.detail} · condiciones de ejecución pendientes`,
  storage:storageFailed?'Registro en memoria; almacenamiento local no disponible':`Registro local · ${journal.length} observaciones (últimas 1000) · ${optionResearchNote()}`};
}



return {readRouteLive,routeChartReading,routeSignalDiagnosticsSnapshot,routeJournal,bindRouteJournal};
}
const selectedReader=createRouteReader();
export const readRouteLive=selectedReader.readRouteLive;
export const routeChartReading=selectedReader.routeChartReading;
export const routeSignalDiagnosticsSnapshot=selectedReader.routeSignalDiagnosticsSnapshot;
export const bindRouteJournal=selectedReader.bindRouteJournal;
export const routeJournal=selectedReader.routeJournal;
