import {escapeHtml as esc, price} from './model';
import {interpretationFresh} from './confirmations';
import type {LiveState} from './live';

const dateET=(t:number)=>new Date(t).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
const timeET=(t:number|null|undefined)=>t==null?'Sin hora':new Date(t).toLocaleTimeString('es',{timeZone:'America/New_York'});
const recent=(t:number|null|undefined,now:number,limit:number)=>t!=null&&Number.isFinite(t)&&t<=now&&now-t<=limit&&dateET(t)===dateET(now);
const number=(n:number|null|undefined)=>n!=null&&Number.isFinite(n)?price(n):'—';
const compact=(n:number|null|undefined)=>n!=null&&Number.isFinite(n)?Intl.NumberFormat('es',{notation:'compact',maximumFractionDigits:2}).format(n):'—';

/** Presentation of existing observations only: no new trading score or entry rule. */
export function intradayReading(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now()):string {
  const own=active&&s?.symbol===symbol&&interpretationFresh(s.publishedMs,now);
  const tick=s?.ticks[symbol], metrics=s?.metrics;
  const trade=own&&s?.stocks.phase==='streaming'&&tick?.symbol===symbol&&recent(tick.tradeMs,now,15000);
  const liveSession=own&&metrics?.session===dateET(now)&&metrics.phase==='Sesión RTH'&&recent(metrics.asofMs,now,180000);
  const last=trade?tick?.price:null, vwap=liveSession?metrics?.vwap:null;
  const relation=last!=null&&Number.isFinite(last)&&vwap!=null&&Number.isFinite(vwap)?last>vwap?'Precio sobre VWAP':last<vwap?'Precio bajo VWAP':'Precio en VWAP':'Esperando precio y VWAP de hoy';
  const flowFresh=own&&s?.options.phase==='streaming'&&recent(s.options.lastEventMs,now,15000);
  const off=s?.offExchange.analytics, session=off?.session;
  const offToday=own&&session?.date===dateET(now);
  const offRecent=offToday&&off?.live_fresh&&recent(off.last_live_ms,now,15000);
  const metric=(label:string,value:string,note:string)=>`<div class="metric"><span>${label}</span><div>${value}</div><p>${note}</p></div>`;
  return `<section class="panel intraday-reading"><div class="panel-head"><div><span class="eyebrow">LECTURA RÁPIDA · SESIÓN ACTUAL · ${esc(symbol)}</span><h2>Qué está pasando hoy</h2></div><span class="tag">${own?esc(metrics?.phase||'Esperando sesión'):'SIN DATOS VIGENTES'}</span></div>
  <p>${esc(relation)}. ${last!=null&&vwap!=null&&Number.isFinite(last)&&Number.isFinite(vwap)?`Distancia ${number(last-vwap)} puntos.`:'Se muestra — cuando falta cobertura o vigencia.'} Esta relación describe el precio; no confirma una entrada.</p>
  <div class="metrics">${metric('Última operación',number(last),trade?`${timeET(tick?.tradeMs)} ET · Stocks`:'Esperando trade propio de hasta 15 s')}${metric('VWAP RTH',number(vwap),liveSession?`Hasta ${timeET(metrics?.asofMs)} ET · minutos del proveedor`:'Esperando sesión RTH y minutos completos')}${metric('RVOL a la misma hora',number(liveSession?metrics?.rvol:null),`${liveSession?metrics?.baselineSessions??0:0} / 5 sesiones históricas válidas`)}${metric('Volumen RTH · acciones',compact(liveSession?metrics?.volume:null),'Acumulado verificado; no contratos de opciones')}</div>
  <p class="panel-caption">${dateET(now)} · Nueva York. Premarket, cierre y referencias de otra fecha no se presentan como sesión regular en curso. VWAP y RVOL conservan las reglas de cobertura del motor; el OI es una referencia del cierre anterior.</p></section>
  <section class="panel"><div class="panel-head"><div><span class="eyebrow">OPCIONES · ACTIVIDAD OBSERVADA</span><h2>Flujo reciente · 1, 5 y 15 minutos</h2></div><span class="tag">CONTRATOS SELECCIONADOS</span></div><div class="table-wrap"><table><thead><tr><th>VENTANA</th><th>CONTRATOS</th><th>PRIMA NEGOCIADA USD</th><th>SESGO INFERIDO</th><th>COBERTURA</th></tr></thead><tbody>${[1,5,15].map(minutes=>{
    const w=s?.flow.windows.find(w=>w.minutes===minutes);
    const valid=flowFresh&&w;
    return `<tr><td>${minutes}m</td><td>${compact(valid?w.contracts:null)}</td><td>${compact(valid&&w.premium_complete?w.premium:null)}</td><td>${valid&&w.event_count>0?esc(w.direction):'Sin evidencia reciente'}</td><td>${valid?'Muestra observada · cobertura total no certificada':'Sin flujo vigente'}</td></tr>`;
  }).join('')}</tbody></table></div><p class="panel-caption">Incluye los vencimientos seleccionados, no solo 0DTE. La prima es volumen negociado, no dinero neto entrando. El lado es inferido con trades y quotes; no identifica apertura/cierre de posiciones. No se recuperan aquí operaciones perdidas durante desconexiones. Ritmo 0DTE y volumen por strike se muestran por separado.</p></section>
  <section class="panel"><div class="panel-head"><div><span class="eyebrow">TRF · SOLO LA FECHA ACTUAL</span><h2>Actividad fuera de bolsa de hoy</h2></div><span class="tag">${offRecent?'PRINT RECIENTE':offToday?'ACUMULADO · SIN PRINT RECIENTE':'ESPERANDO DATOS DE HOY'}</span></div>
  <div class="metrics">${metric('Nocional observado USD',compact(offToday?session?.total_notional:null),offToday&&session?.complete?'Historial conciliado':'Cobertura parcial o pendiente')}${metric('Operaciones observadas',compact(offToday?session?.print_count:null),'Histórico de hoy + live, según cobertura')}${metric('Concentración principal',number(offToday?session?.top_levels[0]?.level:null),'Nivel de mayor nocional observado')}${metric('Último print de hoy',offToday&&off?.last_live_ms!=null&&off.last_live_ms<=now&&dateET(off.last_live_ms)===dateET(now)?timeET(off.last_live_ms):'—','Hora ET · no representa una orden pendiente')}</div>
  <p class="panel-caption">Incluye ATS y no ATS; no demuestra compra institucional ni soporte garantizado. Acumulado de la fecha ET, que puede incluir horario extendido. Las correcciones y la conciliación pueden modificarlo. El histórico de otras sesiones permanece en Lectura del activo.</p></section>`;
}
