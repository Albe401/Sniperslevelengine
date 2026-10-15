import {escapeHtml as esc,price} from './model';
import {interpretationFresh} from './confirmations';
import type {StructuralReading,Alignment} from './reading';
export interface AdjustedReading {reading:StructuralReading&{confidence:number|null};flowApplied:boolean;trfApplied:boolean;trfConfidencePoints:number;trfScorePoints:number;reason:string}
export function adjustedReadingPanel(rows:Record<string,AdjustedReading>|undefined,alignment:Alignment|null|undefined,active:boolean,published:number|undefined,now=Date.now()):string {
  const fresh=active&&interpretationFresh(published,now);
  const value=(n:number|null)=>n==null?'—':price(n);
  return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">ESTRUCTURA + EVIDENCIA EN VIVO</span><h2>Lectura integrada</h2></div><span class="tag">${fresh&&alignment?esc(alignment.status):'Esperando evidencia'}</span></div>
  <div class="table-wrap"><table><thead><tr><th>HORIZONTE</th><th>LECTURA</th><th>SCORE CALL / PUT</th><th>CONFIANZA DEL MODELO</th><th>AJUSTES APLICADOS</th></tr></thead><tbody>${Object.entries(rows??{}).map(([key,r])=>{
    const valid=fresh&&r.reading.available;
    return `<tr><td>${esc(key)}</td><td>${valid?esc(r.reading.reading):'Esperando datos'}</td><td>${valid?`${value(r.reading.callScore)} / ${value(r.reading.putScore)}`:'—'}</td><td>${valid?value(r.reading.confidence):'—'}</td><td>${valid?`${r.flowApplied?'Flujo DAY':'Sin ajuste de flujo'} · ${r.trfApplied?`TRF ${price(r.trfConfidencePoints)} pts`:'Sin ajuste TRF'}`:'—'}<small class="exposure-dates">${valid?esc(r.reason):'Sin evidencia vigente'}</small></td></tr>`;
  }).join('')||'<tr><td colspan="5">Esperando horizontes de opciones y evidencia verificada.</td></tr>'}</tbody></table></div>
  <div class="panel-caption">DAY combina estructura 75 % y flujo 25 % cuando existe cobertura válida. El ajuste TRF usa un máximo de ±12 puntos en DAY/próxima sesión, ±6 en semana y ±3 en próximos cuatro vencimientos; modifica puntuaciones sin invertir el lado previo. Sin evidencia vigente conserva la base estructural. Confianza es una puntuación del modelo, no probabilidad de acierto ni permiso de entrada.</div></section>`;
}
