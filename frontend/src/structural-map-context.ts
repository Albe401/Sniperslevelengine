import type {LiveState} from './live';
import {gexWindowComparisons} from './gex-windows';
import type {MarketEvidence,PressureEvidence} from './market-evidence';
import type {PriceInterval,PriceStructureCard} from './price-structure';
import {readMassiveExposure,massiveExposureIdentity,type MassiveExposureView} from './massive-exposure';

/** Data-association contract only. No combined score, prediction, requests, timers or UI. */
export const STRUCTURAL_CONTEXT_POLICY=Object.freeze({publicationMaxAgeMs:15000,activityMaxAgeMs:3000,activityMinutes:5});
const intervals:readonly PriceInterval[]=[15,60,240,1440,10080];
type Status='available'|'partial'|'pending'|'rejected';
type Freshness='current'|'historical'|'unavailable';
interface SourceState {status:Status;freshness:Freshness;reason:string}
export interface StructureContextInput {symbol:string;publishedMs:number;evaluatedMs:number;cards:readonly PriceStructureCard[]}
export interface StructureLane extends SourceState {
 interval:PriceInterval;side:PriceStructureCard['side'];closedAtMs:number|null;reference:boolean;
 atr:number|null;lastClose:number|null;unit:'USD';
}
export interface OptionScope {
 mode:string;expirations:string[];observedMs:number;populationKeys:string[];
 /** Expiry overlap is evidence of dependence, never permission to sum horizons. */
 overlaps:{mode:string;expirations:string[]}[];
}
export interface OptionsContext extends SourceState {
 source:'Massive';role:'options-context';id:string|null;mode:string|null;expiry:string|null;observedMs:number|null;
 spot:number|null;spotTimeMs:null;oiTimeMs:null;net:number|null;unit:'USD-per-1pct';
 gammaUsed:number|null;eligible:number|null;countCoveragePct:number|null;
 /** Count coverage does not bound missing gamma magnitude. */
 riskCoveragePct:null;priceDirectionInferred:false;profile:MassiveExposureView|null;
}
export interface ActivityContext extends SourceState {
 role:'recent-activity';intervalMinutes:5;observedMs:number|null;signal:number|null;classifiedCoveragePct:number|null;
 unit:'normalized-premium'|'normalized-share-volume';temporalCoverage:'complete'|'not-proven'|'unavailable';
 assignableStructureIntervals:PriceInterval[];
}
export interface StructuralMapContext {
 version:'structural-map-context-v1';symbol:string;atMs:number;framePublishedMs:number|null;frame:SourceState;
 price:StructureLane[];options:OptionsContext;gexWindows?:MassiveExposureView[];optionScopes:OptionScope[];activity:{flow:ActivityContext;cvd:ActivityContext};
 strikeFlow:{status:Status;reason:string;observedMs:number|null;cells:number|null;receptionCoveragePct:null;directionSupportAllowed:false};
 /** One shared context reference, not five independent votes. */
 links:{interval:PriceInterval;contextId:string;role:'context-only';freshness:Freshness}[];
 probability:null;directionScore:null;
}
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v);
const clock=(v:unknown):v is number=>finite(v)&&Number.isSafeInteger(v)&&v>0&&v<=8640000000000000;
const dateET=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
const validDate=(s:string)=>/^\d{4}-\d{2}-\d{2}$/.test(s)&&Number.isFinite(Date.parse(s+'T12:00:00Z'))&&new Date(s+'T12:00:00Z').toISOString().slice(0,10)===s;
const count=(n:unknown):n is number=>Number.isSafeInteger(n)&&Number(n)>=0;
const missingLane=(interval:PriceInterval,reason:string,status:Status='pending'):StructureLane=>({interval,side:'SIN DATOS',closedAtMs:null,reference:false,atr:null,lastClose:null,unit:'USD',status,freshness:'unavailable',reason});

function readLanes(input:StructureContextInput|undefined,symbol:string,published:number|null,now:number,frame:SourceState):StructureLane[] {
 const fail=(reason:string)=>intervals.map(i=>missingLane(i,reason,'rejected'));
 if(frame.status==='rejected')return fail(frame.reason);
 if(!input)return intervals.map(i=>missingLane(i,'Sin salida estructural asociada a esta evaluación'));
 if(input.symbol!==symbol||input.publishedMs!==published||input.evaluatedMs!==now)return fail('La estructura pertenece a otro activo, publicación o evaluación');
 if(!Array.isArray(input.cards)||input.cards.length!==5||input.cards.some(c=>!c||!intervals.includes(c.interval))||new Set(input.cards.map(c=>c.interval)).size!==5)return fail('Se requieren cinco escalas únicas y canónicas');
 return intervals.map(interval=>{
  const c=input.cards.find(c=>c.interval===interval)!;
  if(c.side==='SIN DATOS')return missingLane(interval,c.reason||'Estructura pendiente');
  const pivotValid=(kind:'high'|'low')=>{const rows=kind==='high'?c.highs:c.lows;return Array.isArray(rows)&&rows.length===2&&rows.every(p=>p&&p.kind===kind&&finite(p.price)&&p.price>0&&clock(p.at)&&clock(p.confirmedMs)&&p.at<p.confirmedMs&&p.confirmedMs<=c.endMs!&&count(p.index)&&p.index<c.bars);};
  if(!['CALL','PUT','MIXTA'].includes(c.side)||!clock(c.endMs)||c.endMs>now||c.endMs>published!||!finite(c.atr)||c.atr<=0||!finite(c.lastClose)||c.lastClose<=0||!count(c.bars)||c.bars<100||!finite(c.seedDifference)||c.seedDifference<0||c.seedDifference>.005||!finite(c.highChange)||!finite(c.lowChange)||!pivotValid('high')||!pivotValid('low')||c.invalidated&&c.side!=='MIXTA')return missingLane(interval,'Salida estructural incoherente o con información futura','rejected');
  return {interval,side:c.side,closedAtMs:c.endMs,reference:c.reference,atr:c.atr,lastClose:c.lastClose,unit:'USD',status:'available',freshness:c.reference?'historical':frame.freshness,reason:c.reason};
 });
}

function readOptions(s:LiveState|undefined,symbol:string,active:boolean,now:number,frame:SourceState):OptionsContext {
 const empty:OptionsContext={source:'Massive',role:'options-context',id:null,mode:null,expiry:null,observedMs:null,spot:null,spotTimeMs:null,oiTimeMs:null,net:null,unit:'USD-per-1pct',gammaUsed:null,eligible:null,countCoveragePct:null,riskCoveragePct:null,priceDirectionInferred:false,profile:null,status:'pending',freshness:'unavailable',reason:'Sin exposición Massive verificable'};
 if(frame.status==='rejected')return {...empty,status:'rejected',reason:frame.reason};
 const v=readMassiveExposure(s,symbol,active,now);
 if(v.reason)return {...empty,reason:v.reason};
 if(!validDate(v.expiry))return {...empty,status:'rejected',reason:'Fecha de vencimiento inválida'};
 const h=s!.optionSnapshotStatus?.horizons?.find(h=>h?.mode===v.mode);
 const spot=v.spot??h?.exposure.spot;
 return {...empty,id:massiveExposureIdentity(v),mode:v.mode,expiry:v.expiry,observedMs:v.at,
  spot:finite(spot)&&spot>0?spot:null,net:v.net,gammaUsed:v.gammaUsed,eligible:v.eligible,countCoveragePct:100*v.gammaUsed/v.eligible,
  profile:v,status:v.complete?'available':'partial',freshness:v.historical?'historical':'current',reason:v.complete?'Población recibida; signo CALL-PUT convencional, no inventario dealer':'Subtotal parcial; cobertura de contratos no equivale a cobertura de riesgo'};
}

function scopes(s:LiveState|undefined,frame:SourceState):OptionScope[] {
 if(frame.status==='rejected'||s?.optionSnapshotStatus?.phase!=='ready')return [];
 const hs=s.optionSnapshotStatus.horizons;if(!Array.isArray(hs)||hs.length>16)return [];
 const valid=hs.filter(h=>h&&['DAY','NEXT_SESSION','WEEK','NEXT4'].includes(h.mode)&&hs.filter(x=>x?.mode===h.mode).length===1&&clock(h.observedMs)&&h.observedMs===s.optionSnapshotStatus.observedMs&&h.observedMs<=s.publishedMs&&Array.isArray(h.expirations)&&h.expirations.length>0&&h.expirations.every(x=>typeof x==='string'&&validDate(x))&&new Set(h.expirations).size===h.expirations.length);
 return valid.map(h=>({mode:h.mode,expirations:[...h.expirations],observedMs:h.observedMs,populationKeys:h.expirations.map(x=>JSON.stringify(['Massive-OI',s.symbol,x,h.observedMs])),overlaps:valid.filter(x=>x!==h).map(x=>({mode:x.mode,expirations:x.expirations.filter(e=>h.expirations.includes(e))})).filter(x=>x.expirations.length>0)}));
}

function activity(kind:'flow'|'cvd',s:LiveState|undefined,e:MarketEvidence|undefined,symbol:string,now:number,frame:SourceState):ActivityContext {
 const empty:ActivityContext={role:'recent-activity',intervalMinutes:5,observedMs:null,signal:null,classifiedCoveragePct:null,unit:kind==='flow'?'normalized-premium':'normalized-share-volume',temporalCoverage:'unavailable',assignableStructureIntervals:[],status:'pending',freshness:'unavailable',reason:'Sin evidencia reciente asociada; no sustituye la ventana de una tarjeta'};
 if(frame.status==='rejected')return {...empty,status:'rejected',reason:frame.reason};
 if(!e)return empty;
 if(e.symbol!==symbol||e.at!==now||e.session!==`${symbol}:${dateET.format(now)}:${s?.startedMs??0}`)return {...empty,status:'rejected',reason:'Actividad de otro activo, sesión o evaluación'};
 const p:PressureEvidence=e[kind];
 if(!p||typeof p!=='object')return {...empty,status:'rejected',reason:'Componente de actividad ausente o inválido'};
 if(p.signal==null)return {...empty,reason:p.reason};
 if(!finite(p.signal)||Math.abs(p.signal)>1||!finite(p.coverage)||p.coverage<70||p.coverage>100||!clock(p.at)||p.at>now||!clock(s?.publishedMs)||p.at>s.publishedMs||now-p.at>STRUCTURAL_CONTEXT_POLICY.activityMaxAgeMs||frame.freshness!=='current'||now-s.publishedMs>STRUCTURAL_CONTEXT_POLICY.activityMaxAgeMs)return {...empty,status:'rejected',reason:'Actividad sin unidades, cobertura o reloj compatibles'};
 const base={...empty,observedMs:p.at,signal:p.signal,classifiedCoveragePct:p.coverage,freshness:'current' as const};
 if(kind==='flow')return p.at!==s?.flow?.asof_ms?{...empty,status:'rejected',reason:'La evidencia y el flujo no comparten captura'}:{...base,status:'partial',temporalCoverage:'not-proven',reason:'La prima clasificada no acredita recepción continua de cinco minutos; requiere cobertura temporal'};
 const c=s?.cvd,v=c?.recentFive;
 if(!c?.available||c.symbol!==symbol||c.date!==dateET.format(now)||!clock(c.at)||c.at>s!.publishedMs||now-c.at>STRUCTURAL_CONTEXT_POLICY.activityMaxAgeMs||!v?.complete||!clock(v.startMs)||!clock(v.endMs)||v.endMs!==p.at||v.endMs>c.at||v.endMs-v.startMs!==300000)return {...empty,status:'rejected',reason:'CVD sin ventana completa de cinco minutos compatible'};
 const total=v.buy+v.sell+v.unknown,coverage=100*(v.buy+v.sell)/total,signal=Math.abs(v.delta)>v.unknown?v.delta/total:0;
 if(![v.buy,v.sell,v.unknown,v.delta,total,coverage,signal].every(finite)||Math.min(v.buy,v.sell,v.unknown)<0||total<=0||Math.abs(v.delta-v.buy+v.sell)>Math.max(1e-6,Math.abs(v.delta)*1e-9)||Math.abs(coverage-p.coverage)>1e-8||Math.abs(signal-p.signal)>1e-8)return {...empty,status:'rejected',reason:'El CVD y la evidencia no corresponden a la misma medición'};
 return {...base,status:'available',temporalCoverage:'complete',reason:'CVD de cinco minutos: contexto inmediato, no CVD de 15m/1h/4h/diario/semanal'};
}

/** Accepts the structure already calculated once for this frame; never rebuilds candles. */
export function buildStructuralMapContext(s:LiveState|undefined,symbol:string,active:boolean,now:number,structure?:StructureContextInput,evidence?:MarketEvidence):StructuralMapContext {
 const published=clock(s?.publishedMs)?s.publishedMs:null;
 const valid=typeof symbol==='string'&&symbol.length>0&&symbol.length<=32&&clock(now)&&s?.symbol===symbol&&published!=null&&published<=now;
 const frame:SourceState=!valid?{status:'rejected',freshness:'unavailable',reason:'Activo, publicación o reloj incompatible'}:active&&now-published!<=STRUCTURAL_CONTEXT_POLICY.publicationMaxAgeMs?{status:'available',freshness:'current',reason:'Publicación del activo vigente'}:{status:'available',freshness:'historical',reason:active?'Publicación histórica':'Data desconectada; referencia histórica'};
 const price=readLanes(structure,symbol,published,now,frame),options=readOptions(s,symbol,active,now,frame);
 const heat=frame.status!=='rejected'?s?.flow?.heatmap:undefined;
 const heatValid=heat?.method==='option-execution-heatmap-v1'&&heat.symbol===symbol&&clock(heat.asofMs)&&heat.asofMs<=now&&heat.asofMs<=published!&&Array.isArray(heat.cells);
 const strikeFlow:StructuralMapContext['strikeFlow']={status:!heat?'pending':!heatValid?'rejected':heat.cells.length?'partial':'pending',reason:!heat?'Sin captura de flujo por strike':!heatValid?'Flujo por strike de otro activo, método o reloj':heat.cells.length?'Celdas observadas; falta validar población, ventana y categorías antes de aportar dirección':'Sin ejecuciones observadas; no equivale a flujo cero',observedMs:heatValid?heat.asofMs:null,cells:heatValid?heat.cells.length:null,receptionCoveragePct:null,directionSupportAllowed:false};
 return {version:'structural-map-context-v1',symbol,atMs:now,framePublishedMs:published,frame,price,options,gexWindows:frame.status==='rejected'?[]:gexWindowComparisons(s,symbol,active,now),optionScopes:scopes(s,frame),activity:{flow:activity('flow',s,evidence,symbol,now,frame),cvd:activity('cvd',s,evidence,symbol,now,frame)},strikeFlow,
  links:options.id?price.filter(p=>p.status==='available').map(p=>({interval:p.interval,contextId:options.id!,role:'context-only',freshness:options.freshness})):[],probability:null,directionScore:null};
}
