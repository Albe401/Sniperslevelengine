import type {PriceStructureHistory} from './price-structure-history';
import type {PriceBar,PriceBarSeries} from './price-structure';
const day=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
const finite=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n);
const week=(date:string)=>{const d=new Date(date+'T12:00:00Z');d.setUTCDate(d.getUTCDate()-(d.getUTCDay()+6)%7);return d.toISOString().slice(0,10);};
/** Official daily aggregates with a dated exchange calendar. Never synthesize a missing day. */
export function longStructureBars(h:PriceStructureHistory|undefined,symbol:string,interval:1440|10080,now:number):PriceBarSeries {
 const fail=(reason:string):PriceBarSeries=>({bars:[],gaps:0,reason});
 if(!h)return fail('Esperando historial diario multianual.');
 if(h.symbol!==symbol||!finite(h.asOfMs)||h.asOfMs>now||day.format(h.asOfMs)!==day.format(now)||!finite(h.throughMs)||h.throughMs>h.asOfMs||!Array.isArray(h.sessions)||!Array.isArray(h.days)||h.sessions.length>1500||h.days.length>1500)return fail('Historial diario ajeno, antiguo o inválido.');
 const sessions=h.sessions;
 if(!sessions.length||sessions.some((s,i)=>!/^\d{4}-\d{2}-\d{2}$/.test(s.date)||![s.openMs,s.closeMs].every(finite)||s.openMs%60000!==0||s.closeMs%60000!==0||s.closeMs<=s.openMs||s.closeMs-s.openMs>86400000||day.format(s.openMs)!==s.date||i>0&&(s.openMs<=sessions[i-1].closeMs||s.date<=sessions[i-1].date)))return fail('Calendario diario inválido.');
 const latest=sessions.filter(s=>s.closeMs<=now).at(-1);
 if(latest?.closeMs!==h.throughMs)return fail('Historial diario pendiente del último cierre.');
 const calendar=new Map(sessions.map(s=>[s.date,s])),rows=new Map<string,PriceBar>();
 for(const b of h.days){
  const s=calendar.get(b.date);
  if(!s||s.closeMs>now||rows.has(b.date)||![b.open,b.high,b.low,b.close,b.volume].every(finite)||Math.min(b.open,b.low,b.close)<=0||b.high<Math.max(b.open,b.low,b.close)||b.low>Math.min(b.open,b.close)||b.volume<0)return fail('Velas diarias inválidas, duplicadas o abiertas.');
  rows.set(b.date,{...b,timestampMs:s.openMs,endMs:s.closeMs,sessionDate:s.date});
 }
 const first=[...rows.keys()].sort()[0];if(!first)return fail('Sin velas diarias cerradas.');
 let bars:PriceBar[]=[],gaps=0;
 if(interval===1440){
  for(const s of sessions){if(s.closeMs>now||s.date<first)continue;const b=rows.get(s.date);if(!b){bars=[];gaps++;}else bars.push(b);}
 }else{
  const groups=new Map<string,typeof sessions>();
  for(const s of sessions){const key=week(s.date);groups.set(key,[...(groups.get(key)??[]),s]);}
  for(const group of groups.values()){
   if(group.at(-1)!.closeMs>now||group[0].date<first)continue;
   const complete=group.map(s=>rows.get(s.date));
   if(complete.some(b=>!b)){bars=[];gaps++;continue;}
   const bs=complete as PriceBar[],last=bs.at(-1)!;
   bars.push({timestampMs:bs[0].timestampMs,endMs:last.endMs,sessionDate:last.sessionDate,open:bs[0].open,high:Math.max(...bs.map(b=>b.high)),low:Math.min(...bs.map(b=>b.low)),close:last.close,volume:bs.reduce((n,b)=>n+b.volume,0)});
  }
 }
 return {bars,gaps,reason:bars.length?'':gaps?'Faltan sesiones en la última secuencia.':'Sin semanas completas cerradas.'};
}
