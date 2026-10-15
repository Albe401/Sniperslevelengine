import {escapeHtml as esc,price} from './model';
import {interpretationFresh} from './confirmations';
export interface CmeStatus {
  phase:string;detail:string;environment:string;contract:string|null;nqPrice:number|null;nqTimestampMs:number|null;
  conversions:{source:string;approximate:boolean;nqLevel:number;qqqLevel:number;ratio:number;oldestPriceMs:number}[];
  reconnects:number;
}
const etDate=(ms:number)=>new Date(ms).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
const recent=(ms:number|null,now:number)=>ms!=null&&Number.isFinite(ms)&&ms<=now&&now-ms<=15_000&&etDate(ms)===etDate(now);
export function cmePanel(state:CmeStatus|undefined,active:boolean,publishedMs:number|undefined,now=Date.now()):string {
  const pending=!state||state.phase==='pending_license';
  const fresh=active&&interpretationFresh(publishedMs,now)&&!!state&&['ready','nq_ready'].includes(state.phase)
    &&['UAT','PROD'].includes(state.environment)&&recent(state.nqTimestampMs,now)&&Number.isFinite(state.nqPrice)&&(state.nqPrice??0)>0;
  const rows=fresh&&state?.phase==='ready'?state.conversions.filter(r=>r.source==='NQ_QQQ_OBSERVED_RATIO'&&r.approximate&&recent(r.oldestPriceMs,now)&&[r.nqLevel,r.qqqLevel,r.ratio].every(n=>Number.isFinite(n)&&n>0)):[];
  return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">FUTUROS CME</span><h2>NQ → QQQ</h2></div><span class="tag">${pending?'SIN LICENCIA':esc(state!.environment)}</span></div>
  <div class="panel-caption">${pending?'Integración preparada. Conexión deshabilitada hasta recibir la licencia.':fresh?`${esc(state!.contract??'')} · NQ ${price(state!.nqPrice!)} · ${esc(state!.detail)}`:'Sin datos CME vigentes.'}</div>
  <div class="table-wrap"><table><thead><tr><th>NIVEL NQ</th><th>QQQ APROXIMADO</th><th>COCIENTE OBSERVADO</th></tr></thead><tbody>${rows.map(r=>`<tr><td>${price(r.nqLevel)}</td><td>≈ ${price(r.qqqLevel)}</td><td>${r.ratio.toFixed(6)}</td></tr>`).join('')||'<tr><td colspan="3">Conversión pendiente de datos vigentes y niveles estructurales.</td></tr>'}</tbody></table></div>
  <div class="panel-caption">Conversión por el cociente QQQ/NQ. Requiere el contrato NQ seleccionado, precios de hasta 15 segundos y desfase máximo de 2 segundos. UAT identifica el entorno de pruebas. No autoriza entradas.</div></section>`;
}
