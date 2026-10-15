import { escapeHtml as esc, price } from './model';

export interface DailyRange {
  ready:boolean; method:string|null; spot:number|null; nearestExpiration:string|null;
  atmStrike:number|null; atmIv:number|null; expectedMoveDollars:number|null;
  expectedMovePct:number|null; snapshotLower:number|null; snapshotUpper:number|null;
  atmStraddle:number|null; straddleDailyEquivalent:number|null;
  ivStraddleDivergencePct:number|null; tradingDaysToExpiration:number|null;
  callTicker:string|null; putTicker:string|null; dataQuality:string; notes:string[];
}
export interface RangeSnapshot {
  phase:string; observedMs:number|null; dailyRange?:DailyRange|null;
  dailyRangeError?:string|null; dailyRangeSource?:string;
}
const etDate=(ms:number)=>new Date(ms).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
export function rangeFresh(snapshot:RangeSnapshot|undefined,now=Date.now()):boolean {
  const t=snapshot?.observedMs;
  return t!=null&&Number.isFinite(t)&&t<=now&&now-t<=15*60_000&&etDate(t)===etDate(now);
}
export function dailyRangePanel(snapshot:RangeSnapshot|undefined,connected:boolean,now=Date.now()):string {
  const r=snapshot?.dailyRange;
  const usable=connected&&snapshot?.phase==='ready'&&rangeFresh(snapshot,now)&&r?.ready;
  const value=(n:number|null|undefined)=>n==null?'—':price(n);
  const status=!connected?'Sin conexión vigente':snapshot?.phase==='failed'?'Consulta fallida':snapshot?.phase!=='ready'?'Esperando snapshot':!rangeFresh(snapshot,now)?'Referencia caducada':!r?.ready?'Datos insuficientes':'Estimación disponible';
  const method=r?.method==='ATM_IV_1D'?'Volatilidad implícita · 1 día':r?.method==='ATM_STRADDLE_DAILY_EQUIVALENT_FALLBACK'?'Prima call + put · alternativa':'Sin método disponible';
  const quality:Record<string,string>={HIGH:'Alta',MEDIUM:'Media',LOW:'Baja',WAITING:'Sin base'};
  return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">CONTEXTO DEL MODELO · SNAPSHOT</span><h2>Rango diario estimado</h2></div><span class="tag">${esc(status)}</span></div>
  <div class="table-wrap"><table><thead><tr><th>REFERENCIA</th><th>MOVIMIENTO ±</th><th>BANDA INFERIOR</th><th>BANDA SUPERIOR</th><th>MÉTODO / DATOS</th></tr></thead><tbody><tr>
  <td>${usable?value(r.spot):'—'}<small class="exposure-dates">${esc(snapshot?.dailyRangeSource||'Sin fuente')}</small></td>
  <td>${usable?`${value(r.expectedMoveDollars)} · ${value(r.expectedMovePct)} %`:'—'}</td>
  <td>${usable?value(r.snapshotLower):'—'}</td><td>${usable?value(r.snapshotUpper):'—'}</td>
  <td>${usable?esc(method):'—'}<small class="exposure-dates">${usable?`Cobertura ${esc(quality[r.dataQuality]??r.dataQuality)}`:'Sin estimación vigente'}</small></td></tr></tbody></table></div>
  <div class="panel-caption">${snapshot?.observedMs?`Consulta ${new Date(snapshot.observedMs).toLocaleString('es',{timeZone:'America/New_York'})} ET. `:''}${esc(snapshot?.dailyRangeError||'')}
  ${usable?`Vencimiento ${esc(r.nearestExpiration??'—')} · strike compartido ${value(r.atmStrike)} · IV ${r.atmIv==null?'—':`${value(r.atmIv*100)} %`} · sesiones hasta vencimiento ${r.tradingDaysToExpiration??'—'}.<br>Call ${esc(r.callTicker??'No disponible')} · Put ${esc(r.putTicker??'No disponible')}.<br>`:''}
  Estimación de un día alrededor del precio de referencia, sin anclaje a la apertura. IV anualizada / √252; alternativa: prima call + put del mismo strike / √sesiones. La IV no incluye fecha de publicación por campo. Las primas requieren cotizaciones REAL-TIME de hasta 60 s al consultar. La vigencia de consulta es de 15 minutos y no acredita la edad de la IV ni del precio de referencia. Cobertura describe datos disponibles, no probabilidad de acierto.<br>
  ${esc(r?.notes.join(' · ')||'Esperando datos del modelo.')} No confirma entradas.</div></section>`;
}
