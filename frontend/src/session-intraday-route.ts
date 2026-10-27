import {publicationCandleClock} from './publication-candle-clock';
import {intradayAtr,type AtrRead} from './intraday-atr';
import {routeAtrContext,type RouteAtrContext} from './route-atr-context';
import type {LiveState} from './live';
import {currentSessionCandles} from './candle-series';
import {readPriceNow} from './price-now';
import {RouteEngine,type RouteLevel,type RouteResult} from './route-engine';
import {RouteCvdTracker} from './route-cvd';
import type {MarketEvidence} from './market-evidence';
import type {MarketSignal} from './market-signals';
import {assessRouteStructure,type RouteStructure} from './route-level-context';
import {escapeHtml as esc} from './model';
const day=(t:number)=>Number.isFinite(t)?new Date(t).toLocaleDateString('en-CA',{timeZone:'America/New_York'}):'';
/** Closed session prices only. A pivot becomes known after two later closed minutes. */
export function sessionRouteLevels(s:LiveState|undefined,now:number):RouteLevel[]{
 const date=day(now),bars=currentSessionCandles(s?.chartHistory,1,publicationCandleClock(s?.publishedMs,now)).filter(b=>b.complete&&b.sessionDate===date);
 const out:RouteLevel[]=[];
 const add=(name:string,price:number)=>out.push({name,price,observed:now,asof:now,expiration:date,provisional:false,role:'barrier',source:'session-price'});
 const session=s?.chartHistory?.sessions.find(v=>v.date===date);
 const opening=bars.filter(b=>session&&b.timestampMs>=session.openMs&&b.timestampMs<session.openMs+900000);
 if(session&&opening.length===15&&opening.every((b,i)=>b.timestampMs===session.openMs+i*60000)){
  add('RANGO APERTURA · ALTO',Math.max(...opening.map(b=>b.high)));
  add('RANGO APERTURA · BAJO',Math.min(...opening.map(b=>b.low)));
 }
 for(let i=2;i<bars.length-2;i++){
  const group=bars.slice(i-2,i+3),b=bars[i];
  if(!group.every((v,j)=>v.timestampMs===group[0].timestampMs+j*60000))continue;
  if(group.every((v,j)=>j===2||v.high<b.high))add(`PIVOTE ALTO · ${b.timestampMs}`,b.high);
  if(group.every((v,j)=>j===2||v.low>b.low))add(`PIVOTE BAJO · ${b.timestampMs}`,b.low);
 }
 return out;
}
export interface IntradayRouteRead {
 version:'session-intraday-v1';symbol:string;at:number;state:'waiting'|'candidate'|'confirmed'|'blocked'|'reached'|'invalidated';side:'CALL'|'PUT'|'PENDIENTE';
 label:string;reason:string;target:number|null;stop:number|null;rewardRisk:number|null;basis:'precio + CVD';executionEnabled:false;targetStructure?:string;levelReading?:string;atr?:RouteAtrContext;
 /** Passive explanations from existing branches; no added veto. */
 blockCodes?:string[];result:RouteResult;signals:MarketSignal[];
}
/** Independent visual strategy: 1m reclaim → later test → continuation; no 4H target. */
export class SessionIntradayRoute {
 private engine=new RouteEngine();private cvd=new RouteCvdTracker();private identity='';private known=new Map<string,number>();private events:MarketSignal[]=[];private eventKey='';private lastRead=0;
 sequenceSnapshot(){return this.engine.sequenceSnapshot();}
 closedBarSnapshot(){return this.engine.closedBarSnapshot();}
 read(s:LiveState|undefined,symbol:string,active:boolean,market:MarketEvidence,now:number,atr?:AtrRead):IntradayRouteRead {
  const date=day(now),identity=`${symbol}:${date}:${s?.startedMs??0}`;
  const reset=()=>{this.engine.reset('Nueva prueba intradía requerida');this.cvd.reset();this.known.clear();this.events=[];this.eventKey='';};
  if(identity!==this.identity||now<this.lastRead){reset();this.identity=identity;}this.lastRead=now;
  const out:IntradayRouteRead={version:'session-intraday-v1',symbol,at:now,state:'waiting',side:'PENDIENTE',label:'INTRADÍA · esperando prueba',reason:'Esperando niveles de precio de la sesión y una secuencia nueva.',target:null,stop:null,rewardRisk:null,basis:'precio + CVD',executionEnabled:false,blockCodes:[],result:{...this.engine.result},signals:[]};
  const price=readPriceNow(s?.stocks&&s.metrics&&s.ticks&&s.options&&s.flow?s:undefined,symbol,active,now),calendar=s?.chartHistory?.sessions.find(v=>v.date===date);
  if(!price.ready||s?.symbol!==symbol||s.metrics.phase!=='Sesión RTH'||s.metrics.session!==date||!calendar||now<calendar.openMs||now>=calendar.closeMs){if(!price.ready)out.blockCodes!.push('price-unready');if(s?.symbol!==symbol)out.blockCodes!.push('symbol-mismatch');if(s?.metrics?.phase!=='Sesión RTH'||s?.metrics?.session!==date)out.blockCodes!.push('session-not-rth');if(!calendar)out.blockCodes!.push('calendar-unavailable');else if(now<calendar.openMs||now>=calendar.closeMs)out.blockCodes!.push('outside-session');reset();out.reason='Precio, conexión o sesión RTH sin vigencia.';out.result={...this.engine.result};return out;}
  const magnitude=atr??intradayAtr(s,symbol,active,now);
  out.atr=routeAtrContext(magnitude,price.spot!,null,null,now);
  const bars=currentSessionCandles(s.chartHistory,1,publicationCandleClock(s.publishedMs,now)).filter(b=>b.complete),last=bars.at(-1);
  if(!last||now-last.timestampMs-60000>90000){out.blockCodes!.push('minute-closes-unavailable');reset();out.reason='Cierres de minuto pendientes o caducados.';out.result={...this.engine.result};return out;}
  const important=market.symbol===symbol&&market.at<=now&&now-market.at<=3000?(market.levels??[]).filter(l=>l.expiration===date&&l.observed<=now&&(l.asof??l.observed)<=now&&now-(l.asof??l.observed)<=300000):[];
  const levels=sessionRouteLevels(s,now).map(l=>{const key=l.name+':'+l.price;const observed=this.known.get(key)??now;this.known.set(key,observed);return {...l,observed};});
  const clean=bars.map(({timestampMs,open,high,low,close,volume})=>({timestampMs,open,high,low,close,volume}));
  const revised=this.engine.bars.some(old=>{const b=clean.find(v=>v.timestampMs===old.timestampMs);return !b||JSON.stringify(old)!==JSON.stringify(b);});
  if(revised){out.blockCodes!.push('history-revised');reset();out.reason='Velas corregidas: esperar una prueba nueva.';out.result={...this.engine.result};return out;}
  if(!this.engine.bars.length){let tail=clean.slice(-20);for(let i=tail.length-1;i>0;i--)if(tail[i].timestampMs-tail[i-1].timestampMs!==60000){tail=tail.slice(i);break;}this.engine.warm(tail,identity);}
  else this.engine.feed(clean.at(-1)!,{identity,now,levels,structure:assessRouteStructure(important.filter(l=>l.observed<=last.timestampMs),last.close,this.engine.candidate?.direction??0,this.engine.candidate?.anchor.price??null,now),options:'Contexto separado',delta:'No usado',blocks:'No usado'});
  const r={...this.engine.result},cvd=this.cvd.read(s,r,symbol,active,last.timestampMs,now);out.result=r;out.signals=this.events.map(v=>({...v}));
  if(r.state==='invalidated'||r.state==='reached'){if(r.terminalCause==='expired')out.blockCodes!.push('sequence-expired');out.state=r.state;out.label=r.state==='reached'?'INTRADÍA · referencia alcanzada':'INTRADÍA · invalidada';out.reason=r.reason;return out;}
  if(!['recovery','test','sustained'].includes(r.state)||![1,-1].includes(r.direction)){out.blockCodes!.push(levels.length?'no-candidate-yet':'session-levels-unavailable');out.reason=levels.length?r.reason:'Esperando rango de apertura o pivotes confirmados de la sesión.';return out;}
  out.side=r.direction===1?'CALL':'PUT';out.state='candidate';out.label=`INTRADÍA ${out.side} · candidato`;
  out.reason=r.reason;out.target=r.target;out.stop=r.stop;
  out.atr=routeAtrContext(magnitude,price.spot!,r.target,r.stop,now);
  const destination=levels.find(l=>l.price===r.target);
  if(destination)out.targetStructure=destination.name.startsWith('PIVOTE ALTO')?'Máximo estructural confirmado':destination.name.startsWith('PIVOTE BAJO')?'Mínimo estructural confirmado':destination.name;
  const structure=assessRouteStructure(important,price.spot!,r.direction,r.level,now);
  out.levelReading=intradayLevelReading(structure);
  if(r.state!=='sustained'||r.target==null||r.stop==null){out.blockCodes!.push(r.state!=='sustained'?'price-sequence-pending':'missing-geometry');return out;}
  const risk=(price.spot!-r.stop)*r.direction,reward=(r.target-price.spot!)*r.direction;
  out.rewardRisk=risk>0&&reward>0?reward/risk:null;
  // Same minimum used by the existing structural strategy; policy, not an optimized edge.
  const flow=market.flow,flowFresh=!!flow&&market.symbol===symbol&&market.at<=now&&now-market.at<=3000&&flow.at!=null&&flow.at<=now&&now-flow.at<=3000&&flow.coverage!=null&&flow.coverage>=70&&flow.coverage<=100&&flow.signal!=null&&Number.isFinite(flow.signal)&&Math.abs(flow.signal)<=1;
  const reasons:string[]=[];const blocked=(code:string,reason:string)=>{out.blockCodes!.push(code);reasons.push(reason);};
  if(structure.confirmationBlocked??(structure.quality==='limited'))blocked('weak-level-backing','Respaldo importante débil, parcial o perdiendo fuerza; otra vela no verifica el respaldo del nivel');
  const obstacle=structure.ahead.find(l=>['call-wall','put-wall','barrier'].includes(l.role??'')&&!l.provisional&&l.strength!=null&&l.strength>=60&&(r.target!-l.price)*r.direction>price.spot!*.0003);
  if(obstacle)blocked('obstacle-before-target',`${obstacle.name} ${obstacle.price.toFixed(2)} · fuerza ${obstacle.strength!.toFixed(1)}/100 ${obstacle.grade} antes de la llegada; esperar reacción del precio`);
  if(risk<=0||reward<=0)blocked('price-outside-geometry','Precio fuera del recorrido válido');
  if(out.rewardRisk==null||out.rewardRisk<1.8)blocked('reward-risk-below-minimum','Recorrido restante inferior a 1,8 veces la invalidación');
  if(price.direction!==r.direction)blocked('price-direction-opposed','Precio actual no acompaña la continuación');
  if(cvd.status!=='aligned')blocked(cvd.status==='opposed'?'cvd-opposed':'cvd-unconfirmed',cvd.status==='opposed'?'CVD reciente en contra':cvd.detail);
  if(flowFresh&&flow.signal!*r.direction<0)blocked('option-flow-opposed','Flujo de opciones vigente en contra');
  if(reasons.length){out.state='blocked';out.label=`INTRADÍA ${out.side} · sin confirmar`;out.reason=reasons.join(' · ');return out;}
  out.state='confirmed';out.label=`INTRADÍA ${out.side} · precio + CVD confirmados`;
  out.reason='Recuperación, prueba y continuación en cierres de 1 min; CVD 5m acompaña en dos cierres distintos. '+(flowFresh?'Flujo de opciones contrastado.':'Opciones sin confirmación comparable; no se presentan como apoyo.')+' Confirmación técnica experimental; sin orden automática.';
  const key=r.key+':confirmed';
  if(key!==this.eventKey){this.eventKey=key;this.events=[...this.events,{id:key,session:identity,availableMs:now,barMs:last.timestampMs,state:'sustained' as const,direction:r.direction,price:r.level,text:`INTRADÍA ${out.side} · precio + CVD`,color:r.direction===1?'#25edac':'#ff6282',support:'aligned' as const,evidenceKey:JSON.stringify([identity,r,cvd.at,cvd.observations])}].slice(-100);}
  out.signals=this.events.map(v=>({...v}));return out;
 }
}
export function intradayLevelReading(s:RouteStructure):string {
 const row=(l:RouteStructure['levels'][number])=>`${l.name} ${l.price.toFixed(2)} · fuerza ${l.strength==null?'Sin datos':l.strength.toFixed(1)+'/100 · '+l.grade}${l.change==null?'':` · cambio ${l.change>0?'+':''}${l.change.toFixed(1)}`}${l.provisional?' · PROVISIONAL':''}${l.evidence?.quality==='partial'?' · evidencia parcial':''}`;
 return s.levels.length?[s.reason,...s.levels.slice().sort((a,b)=>b.price-a.price).map(row)].join(' | '):'Niveles importantes sin datos vigentes: respaldo pendiente.';
}
export function intradayRoutePanel(r:IntradayRouteRead|undefined):string {
 const value=(n:number|null|undefined)=>n==null?'Pendiente':`$${n.toFixed(2)}`;
 return `<section class="route-intraday-signal ${r?.state==='confirmed'?(r.side==='CALL'?'intraday-call':'intraday-put'):''}" aria-label="Confirmación intradía"><strong>${esc(r?.label??'INTRADÍA · esperando datos vigentes')}</strong><small>${esc(r?.reason??'Conecta el activo para observar precio y CVD de la sesión.')}</small>${r?.target!=null?`<small>Posible llegada ${value(r.target)} · ${esc(r.targetStructure??'Referencia estructural de la sesión')} · invalida: cierre ${r.result.direction===1?'bajo':'sobre'} ${value(r.stop)}${r.rewardRisk==null?'':` · recorrido / invalidación ${r.rewardRisk.toFixed(2)}`}</small>`:''}${r?.atr?`<small>${esc(r.atr.detail)}</small>`:''}${r?.levelReading?`<small>${esc(r.levelReading)}</small>`:''}<small>Lectura técnica experimental · sin ejecución automática</small></section>`;
}
