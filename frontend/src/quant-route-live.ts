import type {LiveState} from './live';
import type {MarketEvidence} from './market-evidence';
import type {RouteResult,RouteLevel} from './route-engine';
import type {IntradayRouteRead} from './session-intraday-route';
import type {AtrRead} from './intraday-atr';
import type {MarketSignal} from './market-signals';
import {probabilityFeatures} from './route-probability';
import {QuantRouteEngine,quantVersion,quantHorizon,finite,type QuantFrame,type QuantModel,type QuantReading} from './quant-route-engine';
import models from './quant-route-models.json';
import {escapeHtml as esc} from './model';
const day=(n:number)=>finite(n)&&n>0?new Date(n).toLocaleDateString('en-CA',{timeZone:'America/New_York'}):'';
const fresh=(t:unknown,now:number,age:number)=>finite(t)&&t<=now&&now-t<=age;
/** Adapter reads existing outputs only. No recalculation of level/control scores. */
export function quantRouteFrame(s:LiveState|undefined,e:MarketEvidence,r:RouteResult,intraday:IntradayRouteRead,atr:AtrRead|undefined,active:boolean,now:number):QuantFrame {
 const spot=e.price.spot??0,levels=e.levels.filter(l=>l.expiration===day(now)&&!l.provisional&&l.observed<=now&&fresh(l.asof??l.observed,now,300000));
 const upper=levels.filter(l=>l.price>spot).sort((a,b)=>a.price-b.price)[0],lower=levels.filter(l=>l.price<spot).sort((a,b)=>b.price-a.price)[0];
 const calendar=s?.chartHistory?.sessions.find(v=>v.date===day(now));
 const frame:QuantFrame={version:quantVersion,symbol:e.symbol,session:day(now),connection:s?.startedMs??0,at:now,closeMs:atr?.endMs??0,spot,upper:upper?.price??0,lower:lower?.price??0,atr:atr?.value??0,features:{},ready:false,reason:'Esperando precio, ATR, presión y dos referencias vigentes'};
 const missing:string[]=[];
 if(!finite(now))missing.push('reloj inválido');
 if(!active||s?.symbol!==e.symbol||e.symbol!==s?.ticks?.[e.symbol]?.symbol)missing.push('activo/conexión no disponibles');
 if(!e.price.ready||!finite(spot)||spot<=0)missing.push('precio verificable pendiente');
 if(!fresh(e.at,now,3000)||!fresh(s?.publishedMs,now,3000))missing.push('publicación no vigente (máximo 3 s)');
 if(s?.stocks?.phase!=='streaming'||s?.metrics?.session!==day(now)||s?.metrics?.phase!=='Sesión RTH')missing.push('sesión regular en vivo pendiente');
 if(!calendar||now<calendar.openMs||now+quantHorizon>=calendar.closeMs)missing.push('calendario o ventana futura de 10 min insuficientes');
 if(!upper||!lower)missing.push('referencia superior/inferior del vencimiento de hoy, completa y vigente, pendiente');
 if(!finite(atr?.value)||atr.value<=0||!fresh(atr.endMs,now,360000))missing.push('ATR de 5 min pendiente o vencido');
 if(missing.length){frame.reason='Captura bloqueada: '+missing.join(' · ');return frame;}
 if(!s||!calendar||!upper||!lower||!atr||!finite(atr.value)||atr.value<=0)return frame;
 const pressure=(v:MarketEvidence['flow'])=>fresh(v.at,now,3000)&&finite(v.signal)&&Math.abs(v.signal)<=1&&finite(v.coverage)&&v.coverage>=70&&v.coverage<=100?v.signal:null;
 const flow=pressure(e.flow),cvd=pressure(e.cvd);
 if(flow===null&&cvd===null){frame.reason=`Captura bloqueada: presión de opciones y CVD sin cobertura suficiente (mínimo 70%, vigente 3 s). Opciones: ${e.flow.reason??'pendiente'}. CVD: ${e.cvd.reason??'pendiente'}`;return frame;}
 const base=probabilityFeatures(s,e,r,now);
 const profile=(l:RouteLevel,k:'oiShare'|'gammaShare'|'volumeOi'|'iv'|'charm'|'vanna')=>l.evidence&&fresh(l.evidence.at,now,300000)?l.evidence[k]:null;
 const vwap=fresh(s.metrics.asofMs,now,180000)&&finite(s.metrics.vwap)&&s.metrics.vwap>0?s.metrics.vwap:null;
 const unique=[...new Map(levels.map(l=>[l.price,l])).values()];
 const feature:Record<string,unknown>={
  stockPressure:cvd,stockCoverage:cvd===null?null:e.cvd.coverage,optionPressure:flow,optionCoverage:flow===null?null:e.flow.coverage,
  vwapDistance:vwap===null?null:(spot-vwap)/atr.value,upperAtr:(upper.price-spot)/atr.value,lowerAtr:(spot-lower.price)/atr.value,
  upperStrength:upper.strength,lowerStrength:lower.strength,upperChange:upper.change,lowerChange:lower.change,
  upperOiShare:profile(upper,'oiShare'),lowerOiShare:profile(lower,'oiShare'),upperGammaShare:profile(upper,'gammaShare'),lowerGammaShare:profile(lower,'gammaShare'),
  upperVolumeOi:profile(upper,'volumeOi'),lowerVolumeOi:profile(lower,'volumeOi'),upperIv:profile(upper,'iv'),lowerIv:profile(lower,'iv'),
  upperCharm:profile(upper,'charm'),lowerCharm:profile(lower,'charm'),upperVanna:profile(upper,'vanna'),lowerVanna:profile(lower,'vanna'),
  rvol:base.rvol,blockPressure:base.blockPressure,dayDeltaShare:base.dayDeltaShare,weekDeltaShare:base.weekDeltaShare,next4DeltaShare:base.next4DeltaShare,
  magnetAtr:unique.find(l=>l.role==='magnet')? (unique.find(l=>l.role==='magnet')!.price-spot)/atr.value:null,
  vacuumFraction:levels.length?unique.filter(l=>l.role==='vacuum-edge').length/unique.length:null,
  routeStage:['waiting','range','recovery','test','sustained','invalidated','reached'].indexOf(r.state),routeDirection:r.direction,
  intradayStage:['waiting','candidate','confirmed','blocked','reached','invalidated'].indexOf(intraday.state),intradayDirection:intraday.side==='CALL'?1:intraday.side==='PUT'?-1:0,
  sessionFraction:(now-calendar.openMs)/(calendar.closeMs-calendar.openMs),
 };
 frame.features=Object.fromEntries(Object.entries(feature).map(([k,v])=>[k,finite(v)?v:null]));frame.ready=true;frame.reason='Lecturas originales conservadas; referencias y variables congeladas por escenario';return frame;
}
export class QuantRouteMonitor {
 readonly engine:QuantRouteEngine;private events:MarketSignal[]=[];private session='';
 constructor(persist=false){this.engine=new QuantRouteEngine(persist);}
 read(frame:QuantFrame,bars:Parameters<QuantRouteEngine['observe']>[1]){
  const identity=`${frame.symbol}:${frame.session}:${frame.connection}`;
  if(identity!==this.session){this.events=[];this.session=identity;}
  const reading=this.engine.observe(frame,bars,(models as QuantModel[]).find(m=>m.symbol===frame.symbol));
  if(reading.state==='signal'&&reading.sampleId&&!this.events.some(e=>e.id===reading.sampleId)){
   this.events.push({id:reading.sampleId,session:identity,availableMs:frame.at,barMs:Math.floor(frame.at/60000)*60000-60000,state:'sustained',direction:reading.side,price:frame.spot,text:`Q ${reading.side>0?'CALL ↑':'PUT ↓'} · ${(Math.max(...reading.probabilities!)*100).toFixed(1)}% · experimental`,color:reading.side>0?'#25edac':'#ff6282',support:'pending',evidenceKey:reading.sampleId});this.events=this.events.slice(-50);
  }
  return {...reading,reason:reading.reason+(this.engine.error?' · '+this.engine.error:''),samples:this.engine.snapshot().length,signals:frame.ready?this.events.map(e=>({...e})):[]};
 }
}
export function quantRoutePanel(r:QuantReading&{samples:number}):string {
 const probability=r.probabilities?`Superior ${(r.probabilities[0]*100).toFixed(1)}% · inferior ${(r.probabilities[1]*100).toFixed(1)}% · ninguna ${(r.probabilities[2]*100).toFixed(1)}%`:'Probabilidades pendientes de entrenamiento y validación';
 return `<div class="route-intraday-signal" data-quant-route><strong>${esc(r.label)}</strong><small>${esc(probability)}</small><small>${esc(r.reason)} · ${r.samples} escenarios registrados</small><small>Motor separado · primer cierre en referencia · 10 min · sin ejecución</small></div>`;
}
