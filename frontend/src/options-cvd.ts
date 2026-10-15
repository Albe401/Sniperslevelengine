import {atrMeter} from './atr-meter';
import {atrEvidence} from './atr-context';
import type {AtrRead} from './intraday-atr';
import type {LiveState} from './live';
import {escapeHtml as esc} from './model';
export function optionsCvd(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now()):{status:string;label:string;detail:string} {
 const pending=(detail:string)=>({status:'pending',label:'CVD + OPCIONES · PENDIENTE',detail});
 const today=new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'}),c=s?.cvd,w=c?.recentFive;
 if(!active||s?.symbol!==symbol||s.stocks?.phase!=='streaming'||s.options?.phase!=='streaming'||s.metrics?.session!==today||!['Sesión RTH','Esperando primera vela RTH completa'].includes(s.metrics?.phase)||!Number.isFinite(s.publishedMs)||s.publishedMs>now||now-s.publishedMs>3000)return pending('Conexión o sesión sin vigencia.');
 const at=s.options.lastEventMs;
 if(at==null||!Number.isFinite(at)||at>now||now-at>15000||!c||!c.available||c.symbol!==symbol||c.date!==today||c.at!==s.publishedMs||!w||!w.complete||w.endMs!==s.flow.asof_ms||w.endMs>c.at||c.at-w.endMs>3000||w.startMs!==w.endMs-300000)return pending('Esperando cinco minutos móviles observados y datos recientes.');
 const f=s.flow.intraday_windows?.find(v=>v.minutes===5);
 if(!f?.premium_complete||!Number.isInteger(f.event_count)||f.event_count<=0)return pending('Flujo comprador del vencimiento de hoy sin ventana completa.');
 const amounts=['CALL BUY','PUT BUY','CALL SELL','PUT SELL','UNKNOWN'].map(k=>f.buckets?.[k]?.premium);
 if(amounts.some(v=>v==null||!Number.isFinite(v)||v<0))return pending('Primas incompletas.');
 const [cb,pb,cs,ps,u]=amounts as number[],total=cb+pb+cs+ps+u,shares=w.buy+w.sell+w.unknown;
 if(![w.buy,w.sell,w.unknown,w.delta,total,shares].every(Number.isFinite)||Math.min(w.buy,w.sell,w.unknown)<0||shares<=0||total<=0||Math.abs(w.delta-(w.buy-w.sell))>Math.max(1e-6,Math.abs(w.delta)*1e-9)||(w.buy+w.sell)/shares<.7||(total-u)/total<.7)return pending('Cobertura inferior al 70% o datos inconsistentes.');
 const stock=w.delta>w.unknown?'CALL':-w.delta>w.unknown?'PUT':null;
 const option=Math.abs(cb-pb)>u&&Math.abs(cb-pb)/total>=.1?(cb>pb?'CALL':'PUT'):null;
 if(!stock||!option)return pending('Sin predominio verificable de acciones o compras de opciones.');
 const agrees=stock===option;
 return {status:agrees?'aligned':'divergent',label:`${agrees?'ALINEADOS':'DIVERGENTES'} · CVD / ${option} BUY`,detail:`Mismos cinco minutos móviles hasta ${new Date(w.endMs).toLocaleTimeString('es',{timeZone:'America/New_York',hour12:false})} ET. CVD ${stock==='CALL'?'comprador':'vendedor'}: ${w.delta.toLocaleString('es')} acciones. Prima CALL BUY $${cb.toFixed(0)} / PUT BUY $${pb.toFixed(0)} · vencimiento de hoy. Predominio comprador exige diferencia superior a prima desconocida y al 10% de toda la prima; ventas permanecen en el denominador. Lado inferido, contratos observados; no activa entradas ni modifica porcentajes.`};
}
export function optionsCvdPanel(s:LiveState|undefined,symbol:string,active:boolean,now:number,atr?:AtrRead,common?:import('./market-evidence').MarketEvidence):string {
 const r=common?{status:common.support==='aligned'?'aligned':common.support==='opposed'?'divergent':'pending',label:common.support==='aligned'?'ALINEADOS · LECTURA COMÚN / CVD 5m':common.support==='opposed'?'DIVERGENTES · LECTURA COMÚN / CVD 5m':'LECTURA COMÚN + CVD · PENDIENTE',detail:common.detail}:optionsCvd(s,symbol,active,now);
 return `<div class="options-cvd options-cvd-${r.status}" aria-label="CVD y compras de opciones"><strong>${esc(r.label)}</strong><details data-horizon-details="options-cvd"><summary class="atr-inline-summary" title="${esc(atr?.context?.detail??'')}">5 min · ${esc(atr?.context?.label??'Confluencia')} ${atrMeter(atr)}</summary>${atrEvidence(atr)}<p>${esc(r.detail)}</p></details></div>`;
}
