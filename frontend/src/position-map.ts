import {escapeHtml as esc,price} from './model';
import {interpretationFresh} from './confirmations';
export interface PositionMap {ready:boolean;reason:string;price:number|null;putWall:number|null;callWall:number|null;pullTarget:number|null;mainMagnetRelation:string;vacuumRelation:string}
export function positionMapPanel(rows:Record<string,PositionMap>|undefined,connected:boolean,publishedMs:number|undefined,now=Date.now()):string {
  const fresh=connected&&interpretationFresh(publishedMs,now);
  const value=(n:number|null)=>n==null?'—':price(n);
  return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">PRECIO ACTUAL + ESTRUCTURA DE OPCIONES</span><h2>Mapa de posición</h2></div><span class="tag">GEOMETRÍA · SIN ENTRADAS</span></div>
  <div class="table-wrap"><table><thead><tr><th>ESCENARIO</th><th>PUT WALL / PRECIO / CALL WALL</th><th>OBJETIVO PULL</th><th>ESTADO</th><th>IMÁN / VACÍO</th></tr></thead><tbody>${Object.entries(rows??{}).map(([key,r])=>`<tr><td>${esc(key)}</td><td>${fresh?`${value(r.putWall)} / ${value(r.price)} / ${value(r.callWall)}`:'—'}</td><td>${fresh?value(r.pullTarget):'—'}</td><td>${fresh&&r.ready?'Geometría válida':'Esperando evidencia'}<small class="exposure-dates">${fresh?esc(r.reason):'Sin conexión vigente'}</small></td><td>${fresh&&r.ready?`${esc(r.mainMagnetRelation)}<small class="exposure-dates">${esc(r.vacuumRelation)}</small>`:'—'}</td></tr>`).join('')||'<tr><td colspan="5">Esperando precio reciente y estructura de opciones.</td></tr>'}</tbody></table></div>
  <div class="panel-caption">Escenarios CALL y PUT independientes: estar dentro de los muros y tener un objetivo delante no selecciona una dirección ni confirma una operación. Precio de hasta 15 s y estructura de hasta 15 minutos de la misma fecha ET. El objetivo es el preferido del pull; el imán principal conserva M1. La confirmación de flujo y el permiso de entrada se evalúan por separado.</div></section>`;
}
