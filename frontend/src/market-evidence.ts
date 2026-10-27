import {publicationCandleClock} from './publication-candle-clock';
import type {LiveState} from './live';
import type {Candle} from './model';
import type {DirectionScore} from './direction-scanner';
import {readPriceNow,type PriceNow} from './price-now';
import {currentSessionCandles} from './candle-series';
import {mapRouteLevels} from './route-level-context';
import {combineDirection} from './direction-model';
export interface PressureEvidence {signal:number|null;coverage:number|null;at:number|null;reason:string}
export interface MarketEvidence {symbol:string;session:string;at:number;key:string;price:PriceNow;levels:ReturnType<typeof mapRouteLevels>;flow:PressureEvidence;cvd:PressureEvidence;score:DirectionScore;direction:number;support:'aligned'|'opposed'|'mixed'|'pending';detail:string}
const finite=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n);
const day=(n:number)=>new Date(n).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
const clamp=(n:number)=>Math.max(-1,Math.min(1,n));
const pending=(reason:string):DirectionScore=>({call:null,put:null,side:'PENDIENTE',state:'Esperando datos',reason,components:[]});
/** The existing price component, shared by control, confluence and route. */
export function marketPriceSignal(bars:Candle[],vwap:number|null|undefined):number|null {
 const latest=bars.at(-1);if(!latest)return null;const pieces:number[]=[];
 if(latest.close!==latest.open)pieces.push(latest.close>latest.open?.35:-.35);
 if(finite(vwap)&&vwap>0){const d=(latest.close-vwap)/vwap*100;pieces.push(Math.abs(d)<=.03?0:clamp(d/.35));}
 const closes=bars.slice(-4).map(b=>b.close);if(closes.length>=3&&closes[0]>0)pieces.push(clamp((closes.at(-1)!-closes[0])/closes[0]*100/.35));
 return pieces.length?pieces.reduce((a,b)=>a+b,0)/pieces.length:null;
}
/** No sums across overlapping windows, maturities, units, or repeated derived indicators. */
export function readMarketEvidence(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now(),input?:{price?:PriceNow;signal?:number}):MarketEvidence {
 const date=finite(now)?day(now):'',price=input?.price??readPriceNow(s?.ticks&&s.stocks&&s.metrics?s:undefined,symbol,active&&finite(now),finite(now)?now:0),levels=(finite(now)&&s?.optionSnapshotStatus?.horizons?.length?mapRouteLevels(s,symbol,active,now):[]).filter(l=>l.expiration===date);
 const missing=(reason:string):PressureEvidence=>({signal:null,coverage:null,at:null,reason});
 let flow=missing('Prima 5m pendiente'),cvd=missing('CVD reciente 5m pendiente'),score=pending('Precio, flujo o estructura pendientes');
 const own=active&&s?.symbol===symbol&&finite(s.publishedMs)&&s.publishedMs<=now&&now-s.publishedMs<=3000&&s.metrics?.session===date&&s.metrics.phase==='Sesión RTH';
 const optionAt=s?.options?.lastEventMs,w=s?.flow?.intraday_windows?.find(w=>w.minutes===5);
 if(own&&s.options?.phase==='streaming'&&finite(optionAt)&&optionAt<=now&&now-optionAt<=15000&&(s.flowExpiry??date)===date&&(s.flow?.asof_ms==null||finite(s.flow.asof_ms)&&s.flow.asof_ms<=now&&now-s.flow.asof_ms<=3000)&&w?.premium_complete&&Number.isInteger(w.event_count)&&w.event_count>0){
  const amounts=['CALL BUY','PUT SELL','PUT BUY','CALL SELL','UNKNOWN'].map(k=>w.buckets?.[k]?.premium);
  if(amounts.every(n=>finite(n)&&n>=0)){const [cb,ps,pb,cs,u]=amounts as number[],bull=cb+ps,bear=pb+cs,total=bull+bear+u,coverage=100*(bull+bear)/total;
   if(finite(total)&&total>0&&finite(coverage)&&coverage>=70)flow={signal:(bull-bear)/total,coverage,at:finite(s.flow.asof_ms)?s.flow.asof_ms:optionAt,reason:'Prima clasificada 5m · lado inferido · desconocida en el denominador'};
  }
 }
 const c=s?.cvd,v=c?.recentFive,p=c?.regular;
 if(own&&s.stocks?.phase==='streaming'&&c?.available&&c.symbol===symbol&&c.date===date&&finite(c.at)&&c.at<=now&&now-c.at<=3000&&p&&finite(p.lastMs)&&p.lastMs<=now&&now-p.lastMs<=15000&&finite(p.sinceMs)&&v?.complete&&finite(v.startMs)&&finite(v.endMs)&&v.endMs<=now&&now-v.endMs<=3000&&v.endMs-v.startMs===300000&&p.sinceMs<=v.startMs&&day(v.startMs)===date){
  const total=v.buy+v.sell+v.unknown,coverage=100*(v.buy+v.sell)/total;
  const minute=new Date(v.startMs).toLocaleTimeString('en-GB',{timeZone:'America/New_York',hour12:false}).slice(0,5);
  if(minute>='09:30'&&[v.buy,v.sell,v.unknown,v.delta,total,coverage].every(finite)&&Math.min(v.buy,v.sell,v.unknown)>=0&&total>0&&coverage>=70&&Math.abs(v.delta-v.buy+v.sell)<=Math.max(1e-6,Math.abs(v.delta)*1e-9))cvd={signal:Math.abs(v.delta)>v.unknown?v.delta/total:0,coverage,at:v.endMs,reason:'CVD de acciones 5m · tramo completo observado · no atribuido a strikes'};
 }
 let bars=!input&&finite(now)?currentSessionCandles(s?.chartHistory,2,publicationCandleClock(s?.publishedMs,now)):[];bars=bars.filter(b=>b.complete&&b.sessionDate===date).slice(-6);
 for(let i=bars.length-1;i>0;i--)if(bars[i].timestampMs-bars[i-1].timestampMs!==120000){bars=bars.slice(i);break;}
 if(!bars.length||now-bars.at(-1)!.timestampMs>300000)bars=[];
 const vw=own&&finite(s.metrics.asofMs)&&s.metrics.asofMs<=now&&now-s.metrics.asofMs<=180000?s.metrics.vwap:null;
 const signal=input?input.signal:marketPriceSignal(bars,vw),tick=s?.ticks?.[symbol],cw=levels.find(l=>l.role==='call-wall'),pw=levels.find(l=>l.role==='put-wall'),magnet=levels.find(l=>l.role==='magnet');
 if(own&&price.ready&&s.stocks?.phase==='streaming'&&tick?.symbol===symbol&&finite(tick.price)&&tick.price>0&&finite(tick.tradeMs)&&tick.tradeMs<=now&&now-tick.tradeMs<=15000&&finite(signal)&&flow.signal!=null&&cw&&pw&&finite(cw.strength)&&finite(pw.strength)&&pw.price<cw.price){
  const spot=tick.price;let structure=spot<pw.price?-.7:spot>cw.price?.7:(pw.strength-cw.strength)/100;
  if(magnet&&Math.abs(magnet.price-spot)>spot*.0003)structure+=Math.sign(magnet.price-spot)*.2;
  for(const b of levels.filter(l=>l.role==='barrier'&&finite(l.strength)&&Math.abs(l.price-spot)/spot<=.005))structure-=Math.sign(b.price-spot)*(b.strength!/100)*.2;
  const raw=.4*signal+.4*flow.signal+.2*clamp(structure),backing=raw>=0?(pw.price<=spot?pw.strength:null):(cw.price>=spot?cw.strength:null);
  score=combineDirection(signal,flow.signal,clamp(structure),backing,levels.some(l=>l.provisional));
 }
 const direction=score.state==='Lectura mixta'?0:score.side==='CALL'?1:score.side==='PUT'?-1:0;
 const comparable=flow.at!=null&&cvd.at!=null&&Math.abs(flow.at-cvd.at)<=3000;
 const support=!direction||!comparable||cvd.signal==null?'pending':cvd.signal===0?'mixed':Math.sign(cvd.signal)===direction?'aligned':'opposed';
 const detail=`Lectura común: ${score.side} · ${score.state}. CVD 5m ${support==='aligned'?'acompaña':support==='opposed'?'contradice':support==='mixed'?'sin predominio':'pendiente o no comparable'}. Concentración por nivel del mismo snapshot; no suma votos ni cambia el porcentaje. ${flow.reason}.`;
 return {symbol,session:`${symbol}:${date}:${s?.startedMs??0}`,at:now,key:JSON.stringify([symbol,date,s?.startedMs,tick?.tradeMs,s?.optionSnapshotStatus?.observedMs,flow,cvd,score.call,score.put,levels.map(l=>[l.name,l.price,l.evidence?.at])]),price,levels,flow,cvd,score,direction,support,detail};
}

export function marketCvdReading(e:MarketEvidence){return {status:e.support==='aligned'?'confirms' as const:e.support==='opposed'?'contradicts' as const:'pending' as const,label:`CVD 5m · ${e.support==='aligned'?'ACOMPAÑA':e.support==='opposed'?'CONTRADICE':'PENDIENTE'}`,detail:e.detail};}
