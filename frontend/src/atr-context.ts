import {atrMeter} from './atr-meter';
import type {LiveState,Horizon} from './live';
import type {AtrRead} from './intraday-atr';
import {escapeHtml as esc} from './model';
export interface AtrContext {label:string;detail:string;moveFive:number|null;rthRangeDaily:number|null;cvd:string}
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v);
export function atrContext(s:LiveState|undefined,symbol:string,active:boolean,a:AtrRead,now=Date.now()):AtrContext {
 const out:AtrContext={label:'ATR · PENDIENTE',detail:'Sin ATR intradía verificable. El ATR mide magnitud; no crea dirección, absorción ni agotamiento.',moveFive:null,rthRangeDaily:null,cvd:'CVD / ATR: ventanas comparables pendientes.'};
 if(!active||s?.symbol!==symbol||s.stocks?.phase!=='streaming'||!finite(s.publishedMs)||s.publishedMs>now||now-s.publishedMs>3000)return out;
 if(a.daily&&finite(a.daily.value)&&a.daily.value>0){out.label=`ATR D · $${a.daily.value.toFixed(2)}`;out.detail=`ATR14 diario $${a.daily.value.toFixed(2)}, referencia ${a.daily.through}. Escala histórica de magnitud; ATR de 5 min pendiente, sin dirección ni agotamiento confirmado.`;}
 const today=new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'}),session=s.chartHistory?.sessions.find(v=>v.date===today);
 if(session&&s.metrics.session===today&&s.metrics.phase==='Sesión RTH'&&a.value!=null&&finite(a.value)&&a.value>0&&finite(a.lastRangeRatio)&&a.lastRangeRatio>=0&&finite(a.endMs)&&a.endMs>=session.openMs&&a.endMs<=session.closeMs&&a.endMs<=now&&now-a.endMs<300000){
  out.label=`ATR · vela ${a.lastRangeRatio.toFixed(2)}×`;
  out.detail=`Última vela RTH cerrada: true range ${a.lastRangeRatio.toFixed(2)} ATR de 5 min. ${a.lastRangeRatio>=1.2?'Rango mayor que la media reciente':a.lastRangeRatio<=.8?'Rango menor que la media reciente':'Rango próximo a la media reciente'}; umbrales descriptivos 1,2 / 0,8, sin señal de giro. Cierre ${new Date(a.endMs).toLocaleTimeString('es',{timeZone:'America/New_York',hour12:false})} ET.`;
  const end=a.endMs,minutes=s.chartHistory?.minutes??[],start=end-360000;
  const rows=minutes.filter(v=>v.timestampMs>=start&&v.timestampMs<end);
  if(rows.length===6&&new Set(rows.map(v=>v.timestampMs)).size===6){const ordered=rows.slice().sort((x,y)=>x.timestampMs-y.timestampMs);if(ordered.every((v,i)=>v.timestampMs===start+i*60000&&[v.open,v.high,v.low,v.close,v.volume].every(finite)&&v.volume>=0&&v.low>0&&v.high>=Math.max(v.open,v.close)&&v.low<=Math.min(v.open,v.close)))out.moveFive=(ordered[5].close-ordered[0].close)/a.value;}
  const c=s.cvd,w=c?.recentFive;
  if(out.moveFive!=null&&c?.available&&c.symbol===symbol&&c.date===today&&c.at===s.publishedMs&&w?.complete&&w.endMs===a.endMs&&w.startMs===a.endMs-300000&&[w.buy,w.sell,w.unknown,w.delta].every(finite)&&Math.min(w.buy,w.sell,w.unknown)>=0&&w.buy+w.sell+w.unknown>0&&(w.buy+w.sell)/(w.buy+w.sell+w.unknown)>=.7&&Math.abs(w.delta-w.buy+w.sell)<1e-6){
   const side=w.delta>w.unknown?1:-w.delta>w.unknown?-1:0;
   out.cvd=side&&out.moveFive!==0?`CVD / avance 5 min: ${Math.sign(out.moveFive)===side?'alineados':'divergentes'} · ${out.moveFive>0?'+':''}${out.moveFive.toFixed(2)} ATR.`:'CVD / avance 5 min: sin predominio verificable.';
  }
 }
 if(a.daily&&finite(a.daily.value)&&a.daily.value>0&&session&&now>=session.openMs&&now<session.closeMs){
  const end=Math.floor(now/60000)*60000,rows=s.chartHistory!.minutes.filter(v=>v.timestampMs>=session.openMs&&v.timestampMs<end).slice().sort((x,y)=>x.timestampMs-y.timestampMs);
  if(rows.length>0&&rows.length===(end-session.openMs)/60000&&rows.every((v,i)=>v.timestampMs===session.openMs+i*60000&&[v.open,v.high,v.low,v.close,v.volume].every(finite)&&v.volume>=0&&v.low>0&&v.high>=Math.max(v.open,v.close)&&v.low<=Math.min(v.open,v.close))){
   const high=Math.max(...rows.map(v=>v.high)),low=Math.min(...rows.map(v=>v.low));out.rthRangeDaily=(high-low)/a.daily.value!;
   out.detail+=` Rango RTH observado: ${out.rthRangeDaily.toFixed(2)} ATR diarios (referencia ${a.daily.through}). Comparación de magnitud, no rango restante ni límite del día.`;
  }
 }
 out.detail+=' '+out.cvd+' Un ATR elevado o reducido no confirma agotamiento; se conserva la lectura de reacción, precio y flujo. No cambia porcentajes ni reglas de órdenes.';
 return out;
}
export function atrEvidence(a:AtrRead|undefined):string {return a?.context?`<p class="atr-context-evidence">${esc(a.context.detail)} ${atrMeter(a)}</p>`:'';}
export function dailyDistance(a:AtrRead|undefined,spot:number,level:number):string {const value=a?.daily?.value;return finite(value)&&value>0&&finite(spot)&&spot>0&&finite(level)&&level>0?`${(Math.abs(level-spot)/value).toFixed(2)} ATR D`:'';}
export function structuralAtr(a:AtrRead|undefined,h:Horizon|undefined,side:string,spot:number|null):{label:string;detail:string} {
 const pending={label:'ATR D · PENDIENTE',detail:'Distancia estructural: ATR diario, precio o muro del horizonte pendientes.'};
 if(!h?.structure?.complete||spot==null||!['CALL','PUT'].includes(side))return pending;
 const wall=side==='CALL'?h.structure.callWall?.strike:h.structure.putWall?.strike;
 if(!finite(wall)||wall<=0)return pending;
 const distance=dailyDistance(a,spot,wall);if(!distance)return pending;
 return {label:distance,detail:`${side} WALL $${wall.toFixed(2)} · distancia absoluta ${distance} · ATR14 diario hasta ${a!.daily!.through}. Referencia del horizonte; no predice alcance ni es ATR semanal o por expiración.${h.expirations.length<4&&h.mode==='NEXT4'?' Menos de cuatro expiraciones disponibles.':''}`};
}
