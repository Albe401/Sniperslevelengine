import {QuantRouteEngine,quantVersion,quantHorizon,type QuantFrame,type QuantBar} from './quant-route-engine';
import {routeLevelDestinations} from './route-level-destinations';
import type {MarketEvidence} from './market-evidence';
import type {LiveState} from './live';
import type {AtrRead} from './intraday-atr';
import {probabilityFeatures} from './route-probability';
import type {RouteResult} from './route-engine';
import {routeEmaContext} from './route-ema-context';
import {currentFourHour} from './route-4h-live';
import {readArrivalProbability,type ArrivalModel,type ArrivalHorizon} from './arrival-probability-engine';
import models from './arrival-probability-models.json';
import {ArrivalObservationCollector} from './arrival-observation';
/** Evaluation of exploratory pairs; kept separate from production model training and orders. */
export class EarlyRouteEvaluation {
 readonly engine:QuantRouteEngine;
 readonly arrivals:ArrivalObservationCollector;
 constructor(persist=false){this.engine=new QuantRouteEngine(persist,'snipers.early-route-evaluation.v1');this.arrivals=new ArrivalObservationCollector(persist);}
 read(s:LiveState|undefined,m:MarketEvidence,r:RouteResult,a:AtrRead|undefined,bars:QuantBar[],active:boolean,now:number){
  const levels=routeLevelDestinations(m,now,a),upper=levels.find(v=>v.direction===1),lower=levels.find(v=>v.direction===-1),day=new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'}),calendar=s?.chartHistory?.sessions.find(v=>v.date===day);
  const fresh=(t:number|undefined|null,age:number)=>t!=null&&Number.isFinite(t)&&t<=now&&now-t<=age;
  const base=s?.stocks&&s.metrics&&s.chartHistory&&Array.isArray(m.score?.components)?probabilityFeatures(s,m,r,now):{};
  for(const [name,pressure] of [['flow',m.flow],['cvd',m.cvd]] as const){
   const valid=fresh(pressure?.at,3000)&&pressure?.signal!=null&&Number.isFinite(pressure.signal)&&Math.abs(pressure.signal)<=1&&pressure.coverage!=null&&Number.isFinite(pressure.coverage)&&pressure.coverage>=70&&pressure.coverage<=100;
   base[name+'Signal']=valid?pressure.signal:null;base[name+'Coverage']=valid?pressure.coverage:null;
  }
  const arrivalAtr=a?.value!=null&&Number.isFinite(a.value)&&a.value>0&&fresh(a.endMs,300000)?a.value:null;
  const ema=routeEmaContext(s,now),four=currentFourHour(m.symbol,now),spot=m.price.spot;
  const distance=(value:number|null)=>value!=null&&spot!=null&&spot>0?(spot-value)/spot*10000:null;
  const features={...base,atrFiveMinutes:a?.value!=null&&Number.isFinite(a.value)&&a.value>0?a.value:null,ema9Bps:distance(ema.ema9),ema20Bps:distance(ema.ema20),ema50Bps:distance(ema.ema50),ema200Bps:distance(ema.ema200),emaCross:ema.available?ema.cross:null,ema9Rebound:ema.available?ema.rebound9:null,ema20Rebound:ema.available?ema.rebound20:null,fourHourSide:four.scenario?.side??null,fourHourTarget:four.scenario?.target??null,fourHourStop:four.scenario?.invalidation??null,fourHourSupported:four.scenario?(four.supported?1:0):null,upperStrength:upper?.strength??null,lowerStrength:lower?.strength??null,upperProvisional:upper?.provisional?1:0,lowerProvisional:lower?.provisional?1:0};
  const contextReady=!!(Number.isFinite(now)&&active&&s?.symbol===m.symbol&&s.stocks?.phase==='streaming'&&fresh(s.publishedMs,3000)&&s.metrics?.phase==='Sesión RTH'&&s.metrics?.session===day&&calendar&&now>=calendar.openMs&&now<calendar.closeMs&&levels.length>0&&m.price.ready&&fresh(m.at,3000));
  const ready=contextReady&&!!upper&&!!lower&&a?.value!=null&&Number.isFinite(a.value)&&a.value>0&&fresh(a.endMs,300000);
  const f:QuantFrame={version:quantVersion,symbol:m.symbol,session:day,connection:s?.startedMs??0,at:now,closeMs:a?.endMs??0,spot:m.price.spot??0,upper:upper?.target??0,lower:lower?.target??0,atr:a?.value??0,features,ready,reason:ready?'Evaluación exploratoria: primer cierre en uno de dos niveles; no es señal de entrada':'Evaluación pendiente de precio, dos niveles, ATR y continuidad de sesión'};
  this.engine.observe(f,bars,undefined,!!calendar&&now+quantHorizon<calendar.closeMs);
  const upcoming=s?.chartHistory?.sessions.filter(v=>v.closeMs>now).sort((a,b)=>a.openMs-b.openMs)??[];
  const arrivalFrames:Parameters<ArrivalObservationCollector['observe']>[1]=[];
  const probabilities=levels.flatMap(level=>(['10m','session','swing2'] as ArrivalHorizon[]).map(horizon=>{
   const deadline=horizon==='10m'?now+quantHorizon:horizon==='session'?calendar?.closeMs:upcoming[1]?.closeMs;
   const matching=(models as ArrivalModel[]).filter(model=>model.symbol===m.symbol&&model.side===level.direction&&model.horizon===horizon);
   const directionalRoute={...r,direction:level.direction,target:level.target,level:level.target,stop:r.direction===level.direction?r.stop:null};
   const targetFeatures=s?.stocks&&s.metrics&&s.chartHistory&&Array.isArray(m.score?.components)?probabilityFeatures(s,m,directionalRoute,now):{};
   for(const key of ['flowSignal','flowCoverage','cvdSignal','cvdCoverage'])targetFeatures[key]=base[key]??null;
   targetFeatures.targetAtr=arrivalAtr==null?null:level.distance/arrivalAtr;
   const frame={symbol:m.symbol,side:level.direction as 1|-1,horizon,at:now,deadline:deadline??0,origin:level.origin,target:level.target,features:{...features,targetBps:level.distance/level.origin*10000,timeRemainingMinutes:deadline==null?null:(deadline-now)/60000,targetStrength:level.strength,targetProvisional:level.provisional?1:0},ready:contextReady&&deadline!=null&&(horizon!=='10m'||!!calendar&&deadline<calendar.closeMs)};
   Object.assign(frame.features,targetFeatures,{atrFiveMinutes:arrivalAtr,targetBps:level.distance/level.origin*10000,targetStrength:level.strength,anchorStrength:level.strength});
   arrivalFrames.push(frame);return {side:level.direction,...readArrivalProbability(frame,matching.length===1?matching[0]:undefined)};
  }));
  this.arrivals.observe(s,arrivalFrames,now);
  const rows=this.engine.snapshot().filter(v=>v.symbol===m.symbol&&v.session===day),count=(outcome:string)=>rows.filter(v=>v.outcome===outcome).length;
  return {probabilities,upper:count('upper'),lower:count('lower'),neither:count('neither'),missing:count('missing'),pending:count('pending'),reason:f.reason,storageError:this.engine.error,
   label:`Evaluación 10 min · superior ${count('upper')} · inferior ${count('lower')} · ninguno ${count('neither')} · no evaluables ${count('missing')} · pendientes ${count('pending')}`};
 }
}
