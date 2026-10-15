import type {MarketEvidence} from './market-evidence';
import type {RouteResult} from './route-engine';
import type {LevelEvidence} from './level-evidence';
import {assessRoute} from './route-assessment';
import {routePercentLabel,type ProbabilityReading} from './route-probability-reading';
export interface MarketSignal {id:string;session:string;availableMs:number;barMs:number;state:RouteResult['state']|'context';direction:number;price:number|null;text:string;color:string;support:MarketEvidence['support'];evidenceKey:string;profile?:LevelEvidence}
/** Live observations only. Repaints never add signals; no historical inference. */
export class MarketSignalTracker {
 private session='';private key='';private stage='';private at=0;private signals:MarketSignal[]=[];
 reset(){this.session='';this.key='';this.stage='';this.at=0;this.signals=[];}
 read(e:MarketEvidence,r:RouteResult,barMs:number|undefined,ready:boolean,probability?:ProbabilityReading):MarketSignal[]{
  if(!ready||barMs==null||!Number.isFinite(barMs)||barMs+60000>e.at||e.at-barMs-60000>90000||!Number.isFinite(e.at)){this.reset();return [];}
  if(this.session!==e.session||e.at<this.at){this.reset();this.session=e.session;}
  this.at=e.at;
  const stages=['recovery','test','sustained','invalidated','reached'];
  if(!stages.includes(r.state)||!r.key||r.at>e.at||e.at-r.at>90000){this.key='';return this.signals.slice();}
  const observed=e.levels?.find(l=>l.price===r.level)?.evidence;
  const profile=observed&&observed.at<=e.at&&e.at-observed.at<=300000?{...observed,expirations:[...observed.expirations]}:undefined;
  const concentration=profile?.oi!=null&&profile.gex!=null?'OI/GEX observado':profile&&(profile.oi!=null||profile.gex!=null)?'perfil parcial':'perfil pendiente';
  const assessment=assessRoute(r,e,e.at);
  const key=JSON.stringify([r.key,r.state,e.score.side,e.support,concentration,assessment.quality]);if(key===this.key)return this.signals.slice();
  const before=this.signals.at(-1),stage=r.key+':'+r.state,context=this.stage===stage;
  const direction=r.direction||before?.direction||0;
  const labels={recovery:direction>0?'RECUPERACIÓN ↑':'RECHAZO ↓',test:'PRUEBA DEL NIVEL',sustained:direction>0?'CONTINUACIÓN ↑':'CONTINUACIÓN ↓',invalidated:'RUTA INVALIDADA',reached:'REFERENCIA ALCANZADA',waiting:'PENDIENTE',range:'PENDIENTE'};
  const agreement=e.score.call==null?'lectura pendiente':!e.direction?'lectura mixta':direction&&e.direction!==direction?'lectura opuesta':e.support==='opposed'?'CVD contrario':e.support==='aligned'?'lectura + CVD acompañan':'CVD pendiente';
  const arrival=r.state==='sustained'&&assessment.quality!=='opposed'&&r.target!=null&&r.stop!=null&&Number.isFinite(r.target)&&Number.isFinite(r.stop)&&r.target>0&&r.stop>0&&(r.target-r.level!)*direction>0&&(r.level!-r.stop)*direction>0
   ?`POSIBLE LLEGADA ${direction>0?'CALL':'PUT'} $${r.target.toFixed(2)} · invalida $${r.stop.toFixed(2)}`:labels[r.state];
  const text=(context?'CONTEXTO':arrival)+' · '+agreement+' · '+(assessment.quality==='opposed'?'presión en contra':assessment.quality==='aligned'?'apoyo observado':'apoyo incompleto')+' · '+concentration+(probability?' · '+routePercentLabel(probability,direction):'');
  const signal:MarketSignal={id:r.key+':'+r.state+':'+e.at,session:e.session,availableMs:e.at,barMs,state:context?'context':r.state,direction,price:r.level,text,color:assessment.quality!=='aligned'||r.state==='invalidated'?'#ffb564':direction>0?'#25edac':'#ff6282',support:e.support,evidenceKey:e.key};
  // A stage is an event. Changes of context require a new closed bar to avoid noise.
  if(context&&before?.barMs===barMs)return this.signals.slice();
  signal.profile=profile;
  this.key=key;this.stage=stage;this.signals=[...this.signals,signal].slice(-100);return this.signals.slice();
 }
}
/** Map availability to actual chart bars, never to an earlier historical pivot. */
export function chartMarketSignals(signals:MarketSignal[],bars:{timestampMs:number}[],symbol:string,now:number,current:boolean,intervalMs=60000){
 if(!current||!Number.isFinite(now)||!bars.length)return [];
 const times=bars.map(b=>b.timestampMs).sort((a,b)=>a-b);
 return signals.filter(s=>s.session.startsWith(symbol+':')&&s.availableMs<=now).flatMap(s=>{
  const time=times.find((t,i)=>t<=s.availableMs&&(times[i+1]==null||s.availableMs<times[i+1]));
  // If no bar exists at observation time, wait instead of backfilling an old bar.
  if(time==null||time+intervalMs<=s.barMs||s.availableMs>=time+intervalMs)return [];
  return [{timestampMs:time,above:s.direction<0,text:s.text,color:s.color,shape:s.state==='invalidated'||s.state==='context'?'square' as const:s.direction>0?'arrowUp' as const:'arrowDown' as const}];
 });
}
