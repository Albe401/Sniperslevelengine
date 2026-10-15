import {escapeHtml as esc,price} from './model';
import {interpretationFresh} from './confirmations';
interface Coverage {levelsRequested:number;levelsCovered:number;coveragePct:number;contracts:number|null;premium:number|null;trades:number|null}
export interface WallFlow {available:boolean;windows:{minutes:number;callZone:Coverage;putZone:Coverage;callPull:Coverage;putPull:Coverage}[]}
export function wallFlowPanel(reports:Record<string,WallFlow>|undefined,active:boolean,published:number|undefined,lastEvent:number|null|undefined,now=Date.now()):string {
  const fresh=active&&interpretationFresh(published,now)&&lastEvent!=null&&lastEvent<=now&&now-lastEvent<=15_000;
  const value=(v:number|null)=>v==null?'—':price(v);
  return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">VOLUMEN OBSERVADO · NIVELES EXACTOS</span><h2>Zonas de muros y objetivos</h2></div><span class="tag">SIN PERMISO DE ENTRADA</span></div>
  <div class="table-wrap"><table><thead><tr><th>HORIZONTE / VENTANA</th><th>ZONA / OBJETIVO</th><th>NIVELES CUBIERTOS</th><th>CONTRATOS</th><th>PRIMA USD</th></tr></thead><tbody>${Object.entries(reports??{}).flatMap(([mode,r])=>r.windows.flatMap(w=>([['Zona call',w.callZone],['Zona put',w.putZone],['Objetivo call',w.callPull],['Objetivo put',w.putPull]] as const).map(([label,c])=>`<tr><td>${esc(mode)} · ${w.minutes}m</td><td>${label}</td><td>${fresh&&r.available?`${c.levelsCovered} / ${c.levelsRequested} · ${price(c.coveragePct)} %`:'Sin evidencia vigente'}</td><td>${fresh&&r.available?value(c.contracts):'—'}</td><td>${fresh&&r.available?value(c.premium):'—'}</td></tr>`))).join('')||'<tr><td colspan="5">Esperando estructura y ventanas de contratos observadas.</td></tr>'}</tbody></table></div>
  <div class="panel-caption">Cada zona incluye el muro y sus strikes adyacentes disponibles. Se exige observar el contrato exacto en cada vencimiento del horizonte. Un contrato con varios nombres se cuenta una sola vez por zona. Las sumas parciales incluyen solo niveles cubiertos; cero observado y dato desconocido son distintos. Esta vista aún no confirma el escenario wall-pull ni una entrada.</div></section>`;
}
