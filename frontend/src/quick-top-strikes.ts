import type {LiveState} from './live';
import {topStrikes} from './top-strikes';
import {mapHorizon} from './reading-display';
import {rangeFresh} from './daily-range';
import {interpretationFresh} from './confirmations';
import {escapeHtml as esc} from './model';
import './quick-details.css';
import './quick-top-strikes.css';
/** Uses the existing DAY/NEXT_SESSION snapshot ranking; never requests a new chain. */
export function quickTopStrikes(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now()):string {
 const snapshot=s?.optionSnapshotStatus,h=mapHorizon(snapshot?.horizons);
 const fresh=!!(active&&s?.symbol===symbol&&interpretationFresh(s.publishedMs,now)&&snapshot?.phase==='ready'&&rangeFresh(snapshot,now)&&h&&h.observedMs===snapshot.observedMs&&rangeFresh({phase:'ready',observedMs:h.observedMs},now));
 const context=fresh&&h?`${h.mode==='NEXT_SESSION'?'Próxima sesión':'Diario'} · ${h.expirations.join(' / ')}`:'Esperando snapshot vigente';
 return `<section class="quick-details quick-top-strikes" aria-label="Contratos destacados CALL y PUT de ${esc(symbol)}"><header class="qd-heading"><h2>Top 5 · contratos por strike</h2><span>${esc(symbol)} · ${esc(context)}</span></header>${topStrikes(h?.structure.topContracts,fresh)}<details data-horizon-details="quick-top-strikes-method"><summary>Cómo se ordenan</summary><p>OI 40% · volumen 40% · |gamma| × OI 20%, normalizados en la cadena seleccionada. Ranking de concentración del snapshot; no identifica compras, ventas ni probabilidad de éxito. Cada fila es un contrato con strike y vencimiento. Si faltan datos, se muestran únicamente los contratos disponibles.</p></details></section>`;
}
