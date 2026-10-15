import {readDealerPositioning,dealerPositioningPanel,type DealerPositioning} from './dealer-positioning';
import {readMarketEvidence,marketPriceSignal} from './market-evidence';
import {readStrikeAbsorption,type StrikeReading} from './strike-absorption';

import {dealerIntradayPanel} from './dealer-intraday';
import {type NeuroContext} from './neuro-context';
import {absorptionPanel} from './absorption-reading';
import {wallNarrative} from './wall-narrative';
import {readIntradayMap,MapWeightTracker,type IntradayMap} from './intraday-map';
import {type LevelDynamics} from './level-dynamics';
import {AdvantageTracker,type Advantage} from './advantage-tracker';
import {intradaySummary} from './intraday-summary';
import {controlOutlook} from './control-outlook';
import {flowControl} from './flow-control';
import {readPriceLevels,type PriceLevels} from './price-levels';
import {readPriceNow,type PriceNow} from './price-now';
import {IntradayDiagnostics,type DiagnosticInput,type Diagnostics} from './intraday-diagnostics';
import {IvTracker,rangeNarrative,type IvObservation,type IvContext} from './intraday-iv';
import type {LiveState,Horizon} from './live';
import type {FlowWindow} from './quick-flow';
import {currentSessionCandles} from './candle-series';
import {interpretationFresh} from './confirmations';
import {rangeFresh} from './daily-range';
import {rangeContextFresh} from './range-context';
import {localVolumeFresh} from './local-volume';
import {escapeHtml as esc,price,type Candle} from './model';

const clamp=(n:number)=>Math.max(-1,Math.min(1,n));
const finite=(n:number|null|undefined):n is number=>n!=null&&Number.isFinite(n);
const positive=(n:number|null|undefined):n is number=>finite(n)&&n>0;
const etDateFormatter=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
const dateET=(t:number)=>etDateFormatter.format(t);
const recent=(t:number|null|undefined,now:number,age=15000)=>finite(t)&&t>0&&t<=now+1000&&now-t<=age&&dateET(t)===dateET(now);
export const ratioSignal=(a:number|null|undefined,b:number|null|undefined)=>finite(a)&&finite(b)&&a>=0&&b>=0&&a+b>0?clamp((a-b)/(a+b)):null;
export interface ControlComponent {name:string;signal:number;weight:number}
export interface Interaction {name:string;level:number;bias:number;action:string;score:number}
export interface ControlReading {
 dealerPositioning?:DealerPositioning;
 gapReading?:import('./gap-engine').GapReading;
 expectedMoveContext?:{method:string|null;at:number;spot:number|null;expiration:string|null;quality:string};
 marketEvidence?:import('./market-evidence').MarketEvidence;
 vannaEngine?:import('./vanna-engine').VannaEngine;
 dealerLink?:import("./dealer-quick-link").DealerQuickLink;
 readingAt?:number;
 atr?:import('./intraday-atr').AtrRead;
 strikeReading?:StrikeReading;
 neuroContext?:NeuroContext;
 participation?:{rvol:number;at:number};
 levelDynamics?:LevelDynamics;
 intradayMap?:IntradayMap;mapWeightChange?:string;advantage?:Advantage;
 mappedLevels?:{name:string;level:number}[];priceNow?:PriceNow;priceLevels?:PriceLevels;flowControl?:ReturnType<typeof flowControl>;
 greekSupport?:LiveState['greekSupport'];
 diagnosticsInput?:DiagnosticInput;diagnostics?:Diagnostics;
 pricePressure?:number;ivObservation?:IvObservation;iv?:IvContext;expectedMove?:number;symbol:string;available:boolean;net:number;strength:number;coverage:number;control:string;tone:string;
 components:ControlComponent[];dominant:Interaction|undefined;intent:string;next:number|null;nextName:string;
 transition:string;momentum:string;range:LiveState['rangeContext'];observation:string;session:string;
}

export const priceSignal=marketPriceSignal;

export function levelInteraction(h:Horizon|undefined,bars:Candle[],spot:number|null):Interaction|undefined {
 const s=h?.structure;if(!s)return;
 const interactions:Interaction[]=[];
 const latest=bars.at(-1),previous=bars.at(-2);
 const touched=(n:number)=>bars.slice(-4).some(b=>b.low<=n*1.0018&&b.high>=n*.9982);
 const add=(name:string,level:number,bias:number,action:string,score:number)=>interactions.push({name,level,bias,action,score:Math.max(0,Math.min(100,score))});
 const put=s.putWall,call=s.callWall,magnet=s.selected.M1;
 if(positive(put?.strike)&&latest&&touched(put.strike)){
  if(latest.close>put.strike&&latest.low<=put.strike*1.0018)add('PUT WALL',put.strike,1,'DEFENDIENDO SOPORTE',72+Math.min(18,Math.max(0,put.score||0)*.18));
  else if(latest.close<put.strike)add('PUT WALL',put.strike,-1,'SOPORTE PERDIDO',78);
 }
 if(positive(call?.strike)&&latest&&touched(call.strike)){
  if(latest.close<call.strike&&latest.high>=call.strike*.9982)add('CALL WALL',call.strike,-1,'RECHAZANDO PRECIO',72+Math.min(18,Math.max(0,call.score||0)*.18));
  else if(latest.close>call.strike)add('CALL WALL',call.strike,1,'RESISTENCIA SUPERADA',78);
 }
 if(positive(magnet)&&latest&&touched(magnet)){
  if(latest.close>magnet)add('IMÁN PRINCIPAL',magnet,1,'RECUPERADO POR COMPRADORES',previous&&previous.close<=magnet*1.0018?84:68);
  else if(latest.close<magnet)add('IMÁN PRINCIPAL',magnet,-1,'PERDIDO POR COMPRADORES',previous&&previous.close>=magnet*.9982?84:68);
  else add('IMÁN PRINCIPAL',magnet,0,'ROTACIÓN EN EL IMÁN',55);
 }
 for(const route of s.routes){
  if(positive(route.barrier)&&finite(route.barrierScore)&&positive(spot)&&Math.abs(route.barrier-spot)/route.barrier*100<=.35)
   add(route.direction==='CALL'?'BARRERA SUPERIOR':'BARRERA INFERIOR',route.barrier,route.direction==='CALL'?-1:1,'OBSTÁCULO INMEDIATO',Math.max(55,route.barrierScore));
 }
 if(!interactions.length&&positive(spot)){
  for(const [name,level] of [['PUT WALL',put?.strike],['IMÁN PRINCIPAL',magnet],['CALL WALL',call?.strike]] as const)
   if(positive(level)&&Math.abs(level-spot)/level*100<=.35)add(name,level,0,'NIVEL MÁS CERCANO',45);
  interactions.sort((a,b)=>Math.abs(a.level-spot)-Math.abs(b.level-spot));
 }
 return interactions.sort((a,b)=>b.score-a.score)[0];
}

/** Descriptive view; execution uses its independent Rust gates. */
export function buildIntradayControl(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now()):ControlReading {
 const today=dateET(now);
 const own=!!(active&&s?.symbol===symbol&&interpretationFresh(s.publishedMs,now));
 const snapshot=own?s?.optionSnapshotStatus:undefined;
 const h=snapshot?.phase==='ready'&&rangeFresh(snapshot,now)?snapshot.horizons.find(h=>h.mode==='DAY'&&h.structure.complete):undefined;
 const tick=own?s?.ticks[symbol]:undefined;
 const spot=tick?.symbol===symbol&&s?.stocks.phase==='streaming'&&recent(tick.tradeMs,now)&&positive(tick.price)?tick.price:null;
 const history=own&&s?.metrics.session===today&&s.metrics.phase==='Sesión RTH'?s.chartHistory:undefined;
 const bars=currentSessionCandles(history,2,now).filter(b=>b.complete&&b.sessionDate===today);
 let closed=recent(bars.at(-1)?.timestampMs,now,300000)?bars.slice(-6):[];
 for(let i=closed.length-1;i>0;i--)if(closed[i].timestampMs-closed[i-1].timestampMs!==120000){closed=closed.slice(i);break;}
 const vwap=own&&s?.metrics.session===today&&recent(s.metrics.asofMs,now,180000)?s?.metrics.vwap:null;
 const flowFresh=own&&s?.options.phase==='streaming'&&recent(s.options.lastEventMs,now);
 const windows=flowFresh?s?.flow.intraday_windows??[]:[];
 const one=windows.find(w=>w.minutes===1),five=windows.find(w=>w.minutes===5);
 const usable=(w:FlowWindow|undefined)=>!!(w&&Number.isInteger(w.event_count)&&w.event_count>0&&w.premium_complete&&finite(w.bullish_pct)&&finite(w.bearish_pct)&&w.bullish_pct>=0&&w.bearish_pct>=0&&w.bullish_pct<=100&&w.bearish_pct<=100&&w.bullish_pct+w.bearish_pct<=100.01);
 const components:ControlComponent[]=[];
 const add=(name:string,signal:number|null,weight:number)=>{if(finite(signal))components.push({name,signal:clamp(signal),weight});};
 add('FLOW 1M',usable(one)?ratioSignal(one?.bullish_pct,one?.bearish_pct):null,24);
 add('FLOW 5M',usable(five)?ratioSignal(five?.bullish_pct,five?.bearish_pct):null,12);
 add('CALL BUY / PUT BUY 1M',usable(one)?ratioSignal(one?.buckets?.['CALL BUY']?.contracts,one?.buckets?.['PUT BUY']?.contracts):null,18);
 add('PRECIO 2M + VWAP',priceSignal(closed,vwap),20);
 const dominant=levelInteraction(h,closed,spot);
 add('NIVEL EN CONTROL',dominant?dominant.bias*dominant.score/100:null,16);
 const pressure=h?.structure.quickPressure;
 add('PRESIÓN QUICK READ',finite(pressure)?ratioSignal(pressure,100-pressure):null,6);
 if(own&&localVolumeFresh(s?.localVolume,symbol,!!flowFresh,s?.publishedMs,now))
  add('RVOL / PARTICIPACIÓN',ratioSignal(s?.localVolume?.call_contracts_1m,s?.localVolume?.put_contracts_1m),4);
 const coverage=components.reduce((n,c)=>n+c.weight,0);
 const net=coverage?components.reduce((n,c)=>n+c.signal*c.weight,0)/coverage:0;
 const available=coverage>0,control=!available?'ESPERANDO DATOS':coverage<45||spot==null?'EVIDENCIA PARCIAL':net>=.15?'COMPRADORES':net<=-.15?'VENDEDORES':'EQUILIBRIO';
 const tone=!available||coverage<45||spot==null?'waiting':net>=.15?'buyers':net<=-.15?'sellers':'balanced';
 let intent='SIN DIRECCIÓN CLARA',next:number|null=null,nextName='';
 const magnet=h?.structure.selected.M1,wall=net>=.15?h?.structure.callWall?.strike:h?.structure.putWall?.strike;
 if(spot!=null&&coverage>=45){
  if(positive(magnet)&&((net>=.15&&spot<magnet)||(net<=-.15&&spot>magnet))){intent='ROTANDO HACIA EL IMÁN';next=magnet;nextName='IMÁN PRINCIPAL';}
  else if(Math.abs(net)>=.15){
   const route=h?.structure.routes.find(r=>r.direction===(net>0?'CALL':'PUT'));
   const barrier=finite(route?.barrierScore)?route?.barrier:null;
   if(positive(barrier)&&(net>0?barrier>spot:barrier<spot)){intent=net>0?'BUSCANDO LIQUIDEZ SUPERIOR':'BUSCANDO LIQUIDEZ INFERIOR';next=barrier;nextName=net>0?'BARRERA SUPERIOR':'BARRERA INFERIOR';}
   else if(positive(wall)&&(net>0?wall>spot:wall<spot)){intent=net>0?'BUSCANDO LIQUIDEZ SUPERIOR':'BUSCANDO LIQUIDEZ INFERIOR';next=wall;nextName=net>0?'CALL WALL':'PUT WALL';}
   else intent='EXPANSIÓN / SIN OBJETIVO CLARO';
  }
  
 }
 if(spot!=null&&coverage>=45&&Math.abs(net)>=.15&&h){
  const route=h.structure.routes.find(r=>r.direction===(net>0?'CALL':'PUT'));
  const candidates:[string,number|null|undefined][]=[['IMÁN PRINCIPAL',h.structure.selected.M1],['CALL WALL',h.structure.callWall?.strike],['PUT WALL',h.structure.putWall?.strike]];
  if(route?.structuralReady)candidates.push(['BARRERA',route.barrier],['OBJETIVO ESTRUCTURAL',route.target],['BORDE DEL VACÍO',net>0?route.vacuumLow:route.vacuumHigh]);
  const ordered=candidates.filter((v):v is [string,number]=>positive(v[1])&&(net>0?v[1]>spot:v[1]<spot)).sort((a,b)=>Math.abs(a[1]-spot)-Math.abs(b[1]-spot));
  next=ordered[0]?.[1]??null;nextName=ordered[0]?.[0]??'';
  intent=next==null?'SIN NIVEL ADELANTE VERIFICADO':'ESCENARIO HACIA NIVEL CERCANO · CONDICIONAL';
 }
  const mappedLevels:{name:string;level:number}[]=[];
 const mapLevel=(name:string,level:number|null|undefined)=>{if(positive(level))mappedLevels.push({name,level});};
 if(h){
  mapLevel('PUT WALL',h.structure.putWall?.strike);mapLevel('CALL WALL',h.structure.callWall?.strike);mapLevel('IMÁN PRINCIPAL',h.structure.selected.M1);
  for(const route of h.structure.routes)if(route.structuralReady){mapLevel(`BARRERA ${route.direction}`,route.barrier);mapLevel(`OBJETIVO ${route.direction}`,route.target);}
 }
 const session=`${symbol}:${dateET(now)}:${s?.startedMs??0}`;
 const diagnosticsInput:DiagnosticInput={session,ready:own&&spot!=null&&coverage>=45,at:now,spot,target:next,net,flow:usable(one)?ratioSignal(one?.bullish_pct,one?.bearish_pct):null,bars:closed,levels:[]};
 if(h&&snapshot?.observedMs!=null){
  const samples:[string,number|null|undefined,number|null|undefined][]=[];
  for(const [name,w] of [['CALL WALL',h.structure.callWall],['PUT WALL',h.structure.putWall]] as const)if(w?.ready)samples.push([name,w.strike,w.score]);
  const magnet=h.structure.selected.M1;
  samples.push(['IMÁN PRINCIPAL',magnet,h.structure.top?.find(row=>row.strike===magnet)?.score]);
  for(const route of h.structure.routes)if(route.structuralReady){
   samples.push([`BARRERA ${route.direction}`,route.barrier,route.barrierScore]);
   samples.push([`VACÍO ${route.direction}`,route.direction==='CALL'?route.vacuumLow:route.vacuumHigh,route.vacuumScore]);
  }
  for(const [name,level,score] of samples)if(positive(level)&&finite(score)&&score>=0&&score<=100)
   diagnosticsInput.levels.push({key:JSON.stringify([session,name,level,h.expirations,h.referenceSource,h.structure.rows]),name,level,score,at:snapshot.observedMs});
 }
 const range=own&&rangeContextFresh(s?.rangeContext,symbol,active,s?.publishedMs,now)?s?.rangeContext:undefined;
 const model=snapshot?.phase==='ready'&&rangeFresh(snapshot,now)&&snapshot.dailyRange?.ready?snapshot.dailyRange:undefined;
 const ivObservation=model&&positive(model.atmIv)&&snapshot?.observedMs!=null?{key:JSON.stringify([symbol,dateET(now),s?.startedMs,model.nearestExpiration,model.atmStrike,model.callTicker,model.putTicker,snapshot.dailyRangeSource]),at:snapshot.observedMs,percent:model.atmIv*100}:undefined;
 const expectedMoveContext=model&&snapshot?.observedMs!=null?{method:model.method,at:snapshot.observedMs,spot:model.spot,expiration:model.nearestExpiration,quality:model.dataQuality}:undefined;
 const priceNow=readPriceNow(s,symbol,active,now);
 const marketEvidence=readMarketEvidence(s,symbol,active,now);
 return {dealerPositioning:readDealerPositioning(s,symbol,active,now),expectedMoveContext,marketEvidence,readingAt:now,strikeReading:readStrikeAbsorption(s,symbol,active,now),pricePressure:spot!=null&&closed.length?clamp((spot-closed.at(-1)!.close)/(spot*.001)):undefined,neuroContext:own?s?.neuroContext:undefined,participation:own&&s?.metrics.phase==='Sesión RTH'&&s.metrics.session===today&&recent(s.metrics.asofMs,now,180000)&&finite(s.metrics.rvol)&&s.metrics.rvol>=0?{rvol:s.metrics.rvol,at:s.metrics.asofMs!}:undefined,levelDynamics:own?s?.levelDynamics:undefined,intradayMap:readIntradayMap(s,symbol,active,priceNow,now),mappedLevels,flowControl:flowControl(s,symbol,active,priceNow,now),priceLevels:readPriceLevels(s,symbol,active,priceNow,now),priceNow:priceNow,greekSupport:own&&s?.greekSupport?.symbol===symbol&&recent(s.greekSupport.at,now,3000)?s.greekSupport:undefined,diagnosticsInput,ivObservation,expectedMove:positive(model?.expectedMoveDollars)?model.expectedMoveDollars:undefined,symbol,available,net,strength:Math.abs(net)*100,coverage,control,tone,components,dominant,intent,next,nextName,range,
  transition:available?'SEÑAL DIRECCIONAL AÚN DÉBIL · PENDIENTE':'ESPERANDO EVIDENCIA',momentum:'ESTABLECIENDO LECTURA',
  observation:JSON.stringify([closed.at(-1)?.timestampMs,one,five,components]),session:`${symbol}:${dateET(now)}:${s?.startedMs??0}`};
}

export class ControlTracker {
 private ivTracker=new IvTracker();
 private mapTracker=new MapWeightTracker();
 private advantageTracker=new AdvantageTracker();
 private diagnosticsTracker=new IntradayDiagnostics();
 private memory?:{session:string;observation:string;candidate:string;count:number;confirmed:string;net:number;at:number;transition:string;momentum:string};
 update(r:ControlReading,now=Date.now()):ControlReading {
  r={...r,mapWeightChange:this.mapTracker.update(r.intradayMap),advantage:this.advantageTracker.update(intradaySummary(r.priceNow,r.priceLevels,r.flowControl),r.session,r.flowControl?.sourceKey??r.observation,now),iv:this.ivTracker.update(r.ivObservation),diagnostics:r.diagnosticsInput?this.diagnosticsTracker.update(r.diagnosticsInput):undefined};
  if(r.marketEvidence){const e=r.marketEvidence,d=e.direction; r={...r,net:e.score.call==null?0:(e.score.call-50)/50,strength:e.score.call==null?0:Math.abs(e.score.call-50)*2,available:e.score.call!=null,control:e.score.call==null?'ESPERANDO DATOS':d>0?'COMPRADORES':d<0?'VENDEDORES':'EQUILIBRIO',tone:e.score.call==null?'waiting':d>0?'buyers':d<0?'sellers':'balanced'};}
  if(!r.available){this.memory=undefined;return r;}
  let m=this.memory;
  if(m&&(m.session!==r.session||now<m.at||now-m.at>15000))m=undefined;
  if(m?.observation===r.observation)return {...r,transition:m.transition,momentum:m.momentum};
  const eligible=r.marketEvidence?!!r.marketEvidence.direction&&r.marketEvidence.score.state!=='Lectura mixta':['COMPRADORES','VENDEDORES'].includes(r.control)&&r.strength>=55&&r.coverage>=45&&Math.abs(r.net)>=.15;
  const count=eligible?(m?.candidate===r.control?m.count+1:1):0;
  let confirmed=m?.confirmed??'';
  let transition=Math.abs(r.net)<.15?'CONTROL EN EQUILIBRIO':'SEÑAL DIRECCIONAL AÚN DÉBIL · PENDIENTE';
  if(eligible){
   transition=!confirmed?(count>=2?'CONTROL INICIAL CONFIRMADO':`ESTABLECIENDO ${r.control}`):confirmed===r.control?'SIN CAMBIO DE CONTROL':`${confirmed} → ${r.control}${count<2?' · CONFIRMANDO':''}`;
   if(count>=2)confirmed=r.control;
  }
  const delta=m?r.net-m.net:0;
  const momentum=!m?'ESTABLECIENDO LECTURA':Math.abs(delta)<.06?'ESTABLE':Math.abs(r.net)<.15?'MIXTO':delta*r.net>0?'AUMENTANDO':'PERDIENDO FUERZA';
  this.memory={session:r.session,observation:r.observation,candidate:eligible?r.control:'',count,confirmed,net:r.net,at:now,transition,momentum};
  return {...r,transition,momentum};
 }
}

const dollars=(n:number|null|undefined)=>finite(n)?`$${price(n)}`:'—';
export function controlPanel(r:ControlReading,_mapIntegrated=false,structuralMarkup=""):string {
 return `<section class="intraday-control control-structured" aria-label="Control intradía"><header><span class="control-kicker">CONTROL INTRADÍA</span><span class="control-live">${r.neuroContext?.available?'LECTURA DESCRIPTIVA':'ESPERANDO DATOS'}</span></header>${dealerIntradayPanel(r.neuroContext,r.symbol,!!r.neuroContext,r.readingAt??Date.now(),r.dealerLink,true)}${structuralMarkup}</section>`;
}
