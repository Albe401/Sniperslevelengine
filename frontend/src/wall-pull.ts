import {escapeHtml as esc} from './model';
import {interpretationFresh} from './confirmations';
export interface WallPull {state:string;ready:boolean;reason:string;supportReady:boolean;pullReady:boolean;staticReady:boolean;flowAligned:boolean;liveRequired:boolean;liveReady:boolean;liveWindowUsed:number;entryConfirmed:boolean}
export function wallPullPanel(rows:Record<string,WallPull>|undefined,active:boolean,published:number|undefined,stockEvent:number|null|undefined,optionEvent:number|null|undefined,now=Date.now()):string {
  const recent=(t:number|null|undefined)=>t!=null&&t<=now&&now-t<=15_000;
  const fresh=active&&interpretationFresh(published,now)&&recent(stockEvent);
  return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">SOPORTE · OBJETIVO · FLUJO</span><h2>Confirmación wall-pull</h2></div><span class="tag">ANÁLISIS · SIN ENTRADAS</span></div>
  <div class="table-wrap"><table><thead><tr><th>ESCENARIO</th><th>ESTADO DEL MODELO</th><th>ESTRUCTURA</th><th>FLUJO DAY</th><th>ZONAS LIVE</th></tr></thead><tbody>${Object.entries(rows??{}).map(([key,r])=>{
    const valid=fresh&&(!r.liveRequired||recent(optionEvent));
    return `<tr><td>${esc(key)}</td><td>${valid?esc(r.state):'ESPERAR'}<small class="exposure-dates">${valid?esc(r.reason):'Sin evidencia vigente'}</small></td><td>${valid&&r.staticReady?'Confirmada':'Pendiente'}</td><td>${!r.liveRequired?'Contexto estructural':valid&&r.flowAligned?'Alineado':'Pendiente'}</td><td>${!r.liveRequired?'No exigidas':valid&&r.liveReady?`${r.liveWindowUsed}m · Cubiertas`:'Pendientes'}</td></tr>`;
  }).join('')||'<tr><td colspan="5">Esperando estructura, precio y cobertura de flujo.</td></tr>'}</tbody></table></div>
  <div class="panel-caption">DAY exige soporte y pull ≥60/100, geometría válida, flujo alineado ≥55 % o dirección equivalente, ambas zonas ≥60 % y objetivo cubierto en 1m o 5m. El flujo direccional usa 30 minutos observados de los contratos seleccionados del vencimiento DAY; una suscripción nueva o interrumpida debe completar otra ventana. Otros horizontes muestran contexto estructural. «Listo» describe el modelo y no autoriza una operación.</div></section>`;
}
