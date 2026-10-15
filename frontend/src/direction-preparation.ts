import type {LiveState} from './live';
import type {ControlReading} from './intraday-control';
import type {DirectionScore} from './direction-scanner';
import {interpretationFresh} from './confirmations';
export interface DirectionOi {symbol:string;observedMs:number;observedDate:string;referenceDate:string|null;expiration:string;contracts:number;missing:number;call:number;put:number;callTop:{ticker:string;strike:number;oi:number}[];putTop:{ticker:string;strike:number;oi:number}[]}
const finite=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n);
export function preparation(s:LiveState|undefined,r:ControlReading,active:boolean,now:number):DirectionScore|null {
 if(!active||!s||s.symbol!==r.symbol||!interpretationFresh(s.publishedMs,now))return null;
 const today=new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
 const parts=new Intl.DateTimeFormat('en-US',{timeZone:'America/New_York',hourCycle:'h23',hour:'2-digit',minute:'2-digit'}).formatToParts(now);
 const minute=Number(parts.find(p=>p.type==='hour')?.value)*60+Number(parts.find(p=>p.type==='minute')?.value);
 // Backend calendar distinguishes holidays and early closes. Never treat wall-clock time alone as an open session.
 if(s.metrics.session!==today||!['Esperando primera vela RTH completa','Sesión RTH'].includes(s.metrics.phase)||minute<240||minute>=960)return null;
 const o=s.directionOi;
 if(!o||o.symbol!==r.symbol||o.observedDate!==today||o.observedMs>now||now-o.observedMs>900000||!finite(o.observedMs)||o.expiration<today||!/^\d{4}-\d{2}-\d{2}$/.test(o.expiration)||!Number.isInteger(o.contracts)||o.contracts<=0||!Number.isInteger(o.missing)||o.missing<0||o.missing>o.contracts||[o.call,o.put].some(n=>!finite(n)||n<0))return null;
 const top=(rows:DirectionOi['callTop'])=>rows.filter(c=>finite(c.strike)&&c.strike>0&&finite(c.oi)&&c.oi>=0).slice(0,3).map(c=>`${c.strike.toFixed(2)} (${c.oi.toLocaleString('en-US')} OI)`).join(', ')||'pendiente';
 const context=`Vence ${o.expiration} · ${o.contracts-o.missing}/${o.contracts} series con OI en el rango consultado. OI CALL ${o.call.toLocaleString('en-US')} / PUT ${o.put.toLocaleString('en-US')}. Strikes CALL: ${top(o.callTop)}. PUT: ${top(o.putTop)}. OI del cierre del último día negociado; fecha exacta ${o.referenceDate??'no suministrada por el proveedor'}. OI no acredita compras nuevas ni dirección.`;
 const phase=minute<570?'Premarket':minute<600?'Apertura':'Sesión';
 const result:DirectionScore={call:null,put:null,side:'PENDIENTE',state:`${phase} · OI listo`,context,reason:context+' Esperando ejecuciones propias clasificadas.',components:[]};
 const tick=s.ticks[r.symbol],at=s.options.lastEventMs;
 if(s.stocks.phase!=='streaming'||tick?.symbol!==r.symbol||!finite(tick.price)||tick.price<=0||!finite(tick.tradeMs)||tick.tradeMs>now+1000||now-tick.tradeMs>15000||s.options.phase!=='streaming'||!finite(at)||at>now+1000||now-at>15000||o.missing/o.contracts>.3)return result;
 // Primary intraday windows are 0DTE; background windows carry an explicit expiry. Do not mix maturities.
 if(o.expiration!==(s.flowExpiry??today))return result;
 const w=s.flow.intraday_windows?.find(w=>w.minutes===1);
 if(!w?.premium_complete||!Number.isInteger(w.event_count)||w.event_count<=0)return result;
 const amounts=['CALL BUY','PUT BUY','CALL SELL','PUT SELL','UNKNOWN'].map(k=>w.buckets?.[k]?.premium);
 if(amounts.some(n=>!finite(n)||n<0))return result;
 const [cb,pb,cs,ps,u]=amounts as number[],total=cb+pb+cs+ps+u;
 if(!finite(total)||total<=0||(total-u)/total<.7||cb+pb<=0)return result;
 // Only buy-side premium seeds the provisional direction; sales/unknown remain in its denominator.
 const signal=(cb-pb)/total;
 const p=r.priceNow?.ready?r.components.find(c=>c.name==='PRECIO 2M + VWAP')?.signal:undefined;
 const price=finite(p)&&Math.abs(p)<=1?p:0;
 const contracts=(kind:string)=>{const n=w.buckets?.[kind]?.contracts;return finite(n)&&n>=0?n.toLocaleString('en-US'):'pendiente';};
 const net=.3*(.8*signal+.2*price); // Preliminary intensity capped at 65/35, not a validated probability.
 const call=Math.round(50+50*net),put=100-call;
 return {call,put,side:call>=60?'CALL':put>=60?'PUT':'NEUTRAL',state:`${phase} · preliminar 1m`,context,reason:context+` Prima compradora CALL $${cb.toFixed(0)} / PUT $${pb.toFixed(0)}. Contratos CALL BUY ${contracts('CALL BUY')} / PUT BUY ${contracts('PUT BUY')}. Ventana completa 1m; intensidad limitada a65%. Precio ${finite(p)?'disponible':'pendiente'}, niveles y confirmación5m pendientes. Más datos pueden reforzar o reducir la lectura.`,components:[{name:'Prima compradora 1m',signal,weight:80},{name:'Precio disponible',signal:price,weight:20}]};
}
