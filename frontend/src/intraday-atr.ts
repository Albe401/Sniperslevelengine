import {atrMeter} from './atr-meter';
import type {LiveState} from './live';
import {escapeHtml as esc} from './model';
export interface DailyAtr {symbol:string;through:string;value:number|null;close:number|null;detail:string}
export interface AtrRead {context?:import('./atr-context').AtrContext;daily?:DailyAtr;value:number|null;percent:number|null;lastRangeRatio:number|null;endMs:number|null;reason:string}
/** ATR14 SMA: 14 true ranges, each with an actual preceding close. No imputed minutes. */
function intradayOnly(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now()):AtrRead {
 const pending=(reason:string):AtrRead=>({value:null,percent:null,lastRangeRatio:null,endMs:null,reason});
 const today=new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
 if(!active||s?.symbol!==symbol||s.stocks.phase!=='streaming'||s.metrics.session!==today||s.metrics.phase!=='Sesión RTH'||!Number.isFinite(s.publishedMs)||s.publishedMs>now||now-s.publishedMs>3000)return pending('ATR: esperando sesión RTH y publicación vigente.');
 const session=s.chartHistory?.sessions.find(v=>v.date===today);
 if(!session||![session.openMs,session.closeMs].every(Number.isFinite)||session.closeMs<=session.openMs||now<session.openMs||now>=session.closeMs)return pending('ATR: calendario de la sesión pendiente.');
 const end=session.openMs+Math.floor((now-session.openMs)/300000)*300000,start=end-15*300000;
 if(start<session.openMs)return pending('ATR: requiere 75 minutos RTH completos de esta sesión.');
 const rows=new Map<number,{open:number;high:number;low:number;close:number}>();
 for(const c of s.chartHistory!.minutes) {
  if(c.timestampMs<start||c.timestampMs>=end)continue;
  if(![c.timestampMs,c.open,c.high,c.low,c.close,c.volume].every(Number.isFinite)||c.timestampMs%60000!==0||c.low<=0||c.volume<0||c.high<Math.max(c.open,c.close)||c.low>Math.min(c.open,c.close)||rows.has(c.timestampMs))return pending('ATR: minutos inválidos o duplicados; no se rellenan huecos.');
  rows.set(c.timestampMs,c);
 }
 const bars:{high:number;low:number;close:number}[]=[];
 for(let t=start;t<end;t+=300000){let high=-Infinity,low=Infinity,close=0;
  for(let m=t;m<t+300000;m+=60000){const c=rows.get(m);if(!c)return pending('ATR: faltan minutos oficiales completos en las últimas 15 velas.');high=Math.max(high,c.high);low=Math.min(low,c.low);close=c.close;}
  bars.push({high,low,close});
 }
 const tr=bars.slice(1).map((b,i)=>Math.max(b.high-b.low,Math.abs(b.high-bars[i].close),Math.abs(b.low-bars[i].close)));
 const value=tr.reduce((a,b)=>a+b,0)/14;
 if(!Number.isFinite(value)||value<=0)return pending('ATR sin rango positivo verificable.');
 const close=bars[14].close;
 return {value,percent:value/close*100,lastRangeRatio:tr[13]/value,endMs:end,reason:'ATR14 · media simple de true range · velas RTH completas de 5 min. Última vela comparada con ATR incluye el cierre previo. Mide magnitud, no dirección ni agotamiento confirmado.'};
}
export function intradayAtr(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now()):AtrRead {
 const result=intradayOnly(s,symbol,active,now),d=s?.dailyAtr;
 const today=new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
 const latest=s?.chartHistory?.sessions.filter(v=>v.closeMs<now).at(-1)?.date;
 if(active&&s?.symbol===symbol&&d?.symbol===symbol&&d.through<=today&&latest===d.through&&d.value!=null&&d.close!=null&&Number.isFinite(d.value)&&d.value>0&&Number.isFinite(d.close)&&d.close>0)result.daily=d;
 return result;
}
export function atrDistance(atr:AtrRead|undefined,spot:number,level:number):string {
 return atr?.value!=null&&Number.isFinite(atr.value)&&atr.value>0&&Number.isFinite(spot)&&Number.isFinite(level)?`${(Math.abs(level-spot)/atr.value).toFixed(2)} ATR · 5 min`:'';
}
export function atrPanel(a:AtrRead):string {
 const d=a.daily;
 const dollars=(v:number|null|undefined)=>v!=null&&Number.isFinite(v)&&v>0?`$${v.toFixed(2)}`:'Pendiente';
 return `<div class="intraday-atr atr-readable" aria-label="ATR intradía"><div class="atr-readable-item"><span title="ATR14 · 5 MIN">ATR 5 min</span><strong>${dollars(a.value)}</strong><div class="atr-readable-gauge"><small>Vela</small>${atrMeter(a,a.context?.label.startsWith('ATR · vela')?a.lastRangeRatio:null,'Última vela / ATR 5 min')}</div></div><div class="atr-readable-item"><span title="ATR14 · DIARIO · histórico · media simple de true ranges · incluye gaps · hasta ${esc(d?.through??'pendiente')}">ATR diario</span><strong>${dollars(d?.value)}</strong><div class="atr-readable-gauge"><small>Día</small>${atrMeter(a)}</div></div><details data-horizon-details="intraday-atr"><summary>Cómo leerlo</summary><p>5 min: tamaño habitual de una vela. Diario: rango histórico completo, incluidos gaps, promediado con 14 true ranges de sesiones cerradas. No es una banda ± ni el movimiento estimado por IV de opciones. Las barras comparan la última vela y el recorrido del día con su ATR; 100% equivale a 1 ATR. No confirma agotamiento ni giro.</p><p>${esc(a.reason)}${d?` ${esc(d.detail)} Sesión ${esc(d.through)}.`:''}${a.context?' '+esc(a.context.detail):''}${a.endMs==null?'':` Cierre de vela: ${new Date(a.endMs).toLocaleTimeString('es',{timeZone:'America/New_York',hour12:false})} ET. Última vela: ${a.lastRangeRatio!.toFixed(2)} ATR.`}</p></details></div>`;
}
