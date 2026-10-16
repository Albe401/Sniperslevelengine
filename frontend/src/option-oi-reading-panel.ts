import type {Heatmap} from './option-oi-heatmap';
import {readOiMap,type OiSideReading} from './option-oi-reading';
import {escapeHtml as esc} from './model';

/** Read-only explanation above the same map; paging and color scale never alter its population. */
export function oiMapReadingPanel(map?:Heatmap){
 if(!map)return '<section class="oi-reading" aria-label="Lectura del interés abierto"><strong>LECTURA DEL MAPA · OI</strong><p>Lectura pendiente de una cadena verificable del activo.</p></section>';
 const reading=readOiMap(map),n=(v:number)=>v.toLocaleString('es',{maximumFractionDigits:0});
 const value=(s:OiSideReading)=>s.oi===null?'Pendiente':`${s.complete?'':'≥ '}${n(s.oi)}`;
 const top=(s:OiSideReading)=>!s.topStrike?'Concentración pendiente':`Mayor OI ${s.complete?'del filtro':'verificado'}: strike ${s.topStrike.strike.toLocaleString('es',{maximumFractionDigits:3})} · ${n(s.topStrike.oi)} contratos${s.topStrike.ties>1?` · ${s.topStrike.ties} strikes empatados`:''}`;
 const explanation=reading.totalPrecisionError?'La suma combinada excede la precisión segura; reparto y comparación pendientes.':reading.leader===null?(reading.total===0?'El OI verificado de ambos lados es cero. No hay un reparto porcentual que calcular.':'Cobertura incompleta: las cifras disponibles son sumas parciales; el reparto y la comparación global quedan pendientes.'):
  reading.leader==='equal'?'CALL y PUT tienen la misma cantidad de contratos abiertos en este filtro.':`${reading.leader==='call'?'CALL':'PUT'} tiene ${n(reading.difference!)} contratos abiertos más que ${reading.leader==='call'?'PUT':'CALL'} en este filtro.`;
 return `<section class="oi-reading" aria-label="Lectura del interés abierto" data-oi-reading="filtered-open-interest-v1"><header><strong>LECTURA DEL MAPA · OI</strong><span>Todo el filtro · ${map.strikes.length} strikes · ${map.expirations.length} vencimientos</span></header><div class="oi-reading-grid">${(['call','put'] as const).map(side=>{
  const s=reading[side],pct=side==='call'?reading.callPct:reading.putPct;
  return `<article class="oi-reading-${side}"><span>${side.toUpperCase()} · CONTRATOS ABIERTOS</span><strong>${value(s)}</strong><small>${s.complete?'Total verificado del filtro':s.precisionError?'Suma fuera de precisión segura':s.oi===null?'Sin suma verificable':'Mínimo verificado · parcial'}${pct!==null?` · ${pct.toFixed(1)}% del OI total`:''}</small><p>${esc(top(s))}</p><small>${s.knownSeries}/${s.expectedSeries} series con OI sumado${s.unresolvedCells?` · ${s.unresolvedCells} celdas pendientes`:''}</small></article>`;
 }).join('')}</div><p class="oi-reading-result">${esc(explanation)}</p><p class="oi-reading-scope">Incluye todos los strikes y vencimientos del filtro en ambos lados, también fuera de la página visible. Contratos estándar; ${map.nonStandard.length} excluidos por entregable no estándar y ${map.unknownDeliverable.length} entregables pendientes. No se suman celdas sin OI verificable.</p><p class="oi-reading-scope">OI diario · fecha efectiva: ${esc(map.oiDate??'no suministrada por la fuente')} · captura ${esc(new Date(map.observedMs).toLocaleString('es',{timeZone:'America/New_York'}))} ET. Cada contrato tiene comprador y vendedor: más CALL o PUT no demuestra compras, ventas ni dirección futura.</p></section>`;
}
