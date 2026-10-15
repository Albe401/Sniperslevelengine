import type {LiveState} from './live';
import {sessionCandles,type ChartHistory} from './candle-series';
import {interpretationFresh} from './confirmations';
import {price,escapeHtml as esc} from './model';
const date=(t:number)=>new Date(t).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
export function recentMovement(history:ChartHistory|undefined,now:number):string {
 const bars=sessionCandles(history,[],2,now).filter(b=>b.sessionDate===date(now));
 const last=bars.filter(b=>b.timestampMs+120000<=now).slice(-2);
 const [a,b]=last;
 if(!a||!b||!a.complete||!b.complete||b.timestampMs-a.timestampMs!==120000||now-(b.timestampMs+120000)>180000)return 'Movimiento 2m pendiente';
 const delta=b.close-a.close;
 return `${delta<0?'Caída reciente':delta>0?'Subida reciente':'Sin cambio entre cierres'} · ${delta>0?'+':''}${price(delta)} puntos (2m)`;
}
export function sessionSignals(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now()):string {
 const fresh=active&&s?.symbol===symbol&&interpretationFresh(s.publishedMs,now);
 const current=(t:number|null|undefined,age:number)=>t!=null&&Number.isFinite(t)&&t<=now&&now-t<=age&&date(t)===date(now);
 const metrics=s?.metrics,tick=s?.ticks[symbol];
 const session=fresh&&s?.stocks.phase==='streaming'&&metrics?.session===date(now)&&metrics.phase==='Sesión RTH'&&current(metrics.asofMs,180000);
 const vwap=session&&metrics?.vwap!=null&&Number.isFinite(metrics.vwap)&&metrics.vwap>0?metrics.vwap:null;
 const spot=session&&current(tick?.tradeMs,15000)&&tick?.price!=null&&Number.isFinite(tick.price)&&tick.price>0?tick.price:null;
 const relation=vwap!=null&&spot!=null?(spot>vwap?'Sobre VWAP':spot<vwap?'Bajo VWAP':'En VWAP'):'Esperando datos de hoy';
 const movement=session?recentMovement(s?.chartHistory,now):'Movimiento 2m pendiente';
 const rows=session?(s?.structuralPaths??[]).filter(r=>r.symbol===symbol&&r.horizon==='DAY'&&r.candidate.available&&!r.candidate.blocks.includes('UNVERIFIED_INPUT')&&current(r.trigger.bar_end_ms,180000)):[];
 const confirmed=rows.filter(r=>r.trigger.ready);
 const trigger=confirmed.length?confirmed.map(r=>`${esc(r.candidate.side)} confirmado`).join(' · '):rows.length?'Esperando confirmación':'Sin vela vigente';
 return `<section class="panel session-signals"><span class="eyebrow">PRECIO · SESIÓN ACTUAL</span><div><span>VWAP de hoy</span><strong>${vwap==null?'—':price(vwap)}</strong><small>${relation}${spot!=null&&vwap!=null?` · ${price(spot-vwap)} puntos`:''}</small></div><small class="recent-price-move">${esc(movement)}</small><div><span>Confirmación del imán · vela 2m</span><strong>${trigger}</strong><small>${confirmed.length?'Vela cerrada; admisión pendiente del estado de decisión.':'Se verifica con la ruta DAY del activo.'}</small></div></section>`;
}
