import type {LiveState} from './live';
import type {DirectionScore} from './direction-scanner';
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v);
const clamp=(v:number)=>Math.max(-1,Math.min(1,v));
/** Price-led premarket intensity; missing CVD reduces amplitude, never supplies a vote. */
export function premarketDirection(s:LiveState|undefined,symbol:string,active:boolean,now:number):DirectionScore|null {
 if(!active||!s||s.symbol!==symbol||!finite(now)||!finite(s.publishedMs)||s.publishedMs>now||now-s.publishedMs>3000||s.stocks.phase!=='streaming')return null;
 const date=new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
 const session=s.chartHistory?.sessions.filter(v=>v.date===date);
 if(session?.length!==1||!finite(session[0].openMs)||now<session[0].openMs-19800000||now>=session[0].openMs||s.metrics.session!==date||s.metrics.phase!=='Esperando primera vela RTH completa')return null;
 const tick=s.ticks[symbol];
 if(tick?.symbol!==symbol||!finite(tick.price)||tick.price<=0||!finite(tick.tradeMs)||tick.tradeMs>now||now-tick.tradeMs>15000)return null;
 const end=Math.floor(now/60000)*60000-60000,start=end-300000;
 if(start<session[0].openMs-19800000)return null;
 const raw=s.minutes?Object.values(s.minutes):s.chartHistory?.minutes??[];
 const closes:number[]=[];
 for(let at=start;at<=end;at+=60000){
  const rows=raw.filter(b=>b.timestampMs===at);
  if(rows.length!==1)return null;
  const b=rows[0];
  if(![b.open,b.high,b.low,b.close,b.volume].every(finite)||b.low<=0||b.volume<0||b.high<Math.max(b.open,b.close)||b.low>Math.min(b.open,b.close))return null;
  closes.push(b.close);
 }
 const price=clamp((closes[5]/closes[0]-1)*100/.35);
 let cvd:number|null=null;
 const c=s.cvd,p=c?.premarket;
 if(c?.available&&c.symbol===symbol&&c.date===date&&finite(c.at)&&c.at<=now&&now-c.at<=3000&&p&&finite(p.lastMs)&&p.lastMs<=now&&now-p.lastMs<=15000&&finite(p.sinceMs)&&p.sinceMs<=now-300000&&p.gaps===0){
  const total=p.buy+p.sell+p.unknown;
  if([p.buy,p.sell,p.unknown,p.delta,total].every(finite)&&Math.min(p.buy,p.sell,p.unknown)>=0&&total>0&&(p.buy+p.sell)/total>=.7&&Math.abs(p.delta-p.buy+p.sell)<=Math.max(1e-6,Math.abs(p.delta)*1e-9))cvd=Math.abs(p.delta)>p.unknown?p.delta/total:0;
 }
 // TRF prints are already part of the consolidated stock tape: never add
 // their volume or an independent positive vote on top of CVD.
 const report=s.offExchange?.analytics?.blocks,b=report?.premarket;
 let blocks:number|null=null;
 if(report?.symbol===symbol&&report.date===date&&b?.complete&&!b.finalized&&b.count>0&&finite(b.through_ms)&&b.through_ms<=now&&now-b.through_ms<=15000&&[b.buy,b.sell,b.unknown,b.total].every(finite)&&Math.min(b.buy,b.sell,b.unknown)>=0&&b.total>0&&Math.abs(b.total-b.buy-b.sell-b.unknown)<=Math.max(.01,b.total*1e-9)&&(b.buy+b.sell)/b.total>=.7){
  blocks=Math.abs(b.buy-b.sell)>b.unknown?(b.buy-b.sell)/b.total:0;
 }
 const conflict=cvd!=null&&blocks!=null&&cvd*blocks<0;
 const flow=cvd==null?.2*(blocks??0):.4*cvd*(conflict?.5:1);
 const call=Math.round(50+15*(.6*price+flow)),put=100-call;
 return {call,put,side:call>=60?'CALL':put>=60?'PUT':'NEUTRAL',state:conflict?'Premarket · conflicto CVD / bloques':cvd==null?(blocks==null?'Premarket · preliminar por precio':'Premarket · precio + bloques TRF'):'Premarket · precio + CVD',components:[{name:'Precio premarket 5m',signal:price,weight:60},...(cvd==null?(blocks==null?[]:[{name:'Bloques TRF premarket',signal:blocks,weight:20}]):[{name:'CVD premarket',signal:cvd,weight:conflict?20:40}])],reason:`Sesgo preliminar de acciones: cambio entre seis cierres consecutivos de un minuto, normalizado a 0,35%. ${cvd==null?(blocks==null?'Sin flujo verificado: intensidad limitada a 59/41.':'Bloques TRF con cobertura ≥70% y periodo completo vigente: aporte limitado a 20%; intensidad máxima 62/38.'):'CVD observado con cobertura ≥70%; intensidad limitada a 65/35.'} ${blocks==null?'Bloques sin cobertura vigente: no aportan dirección.':cvd==null?'Bid/ask estima agresión; no identifica instituciones.':conflict?'Bloques contrarios reducen a la mitad el aporte CVD.':'Bloques contrastados sin sumar otro voto al CVD.'} No se suma volumen TRF al CVD. Sin flujo de opciones ni probabilidad de éxito. Se sustituye por la lectura RTH al abrir.`};
}
