import { escapeHtml as esc } from './model';
export interface DexChange {
  available:boolean;side:string;reason:string;call:number|null;put:number|null;
  net:number|null;pctGross:number|null;previousNet:number|null;previousMs:number|null;observedMs:number;
}
interface Snapshot {phase:string;dexChanges?:Record<string,DexChange>}
const maxAge=15*60_000;
const etDate=(ms:number)=>new Date(ms).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
export function dexUsable(change:DexChange,now:number):boolean {
  return change.available && Number.isFinite(change.observedMs) && change.observedMs<=now
    && now-change.observedMs<=maxAge && etDate(change.observedMs)===etDate(now);
}
export function dexFreshnessSignature(snapshot:Snapshot|undefined,now=Date.now()):boolean[] {
  return Object.values(snapshot?.dexChanges??{}).map(c=>dexUsable(c,now));
}
export function dexChangePanel(snapshot:Snapshot|undefined,connected:boolean,now=Date.now()):string {
  const number=(n:number|null)=>n==null?'—':Intl.NumberFormat('es',{maximumFractionDigits:3,notation:'compact'}).format(n===0?0:n);
  const rows=Object.entries(snapshot?.dexChanges??{});
  return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">VARIACIÓN ENTRE SNAPSHOTS</span><h2>DEX Change</h2></div><span class="tag">CONTEXTO · SIN ENTRADAS</span></div>
  <div class="table-wrap"><table><thead><tr><th>HORIZONTE</th><th>ESTADO</th><th>Δ CALL / PUT</th><th>Δ NETO</th><th>% DEX BRUTO</th></tr></thead><tbody>${rows.map(([mode,c])=>{
    const usable=connected&&snapshot?.phase==='ready'&&dexUsable(c,now);
    const label=usable?c.side:!connected?'Desconectado':snapshot?.phase!=='ready'?'Snapshot pendiente':!c.available?'Sin base comparable':'Referencia caducada';
    return `<tr><td>${esc(mode)}<small class="exposure-dates">Observación ${new Date(c.observedMs).toLocaleTimeString('es',{timeZone:'America/New_York'})} ET</small></td><td>${esc(label)}<small class="exposure-dates">${esc(c.reason)}</small></td><td>${usable?`${number(c.call)} / ${number(c.put)}`:'—'}</td><td>${usable?number(c.net):'—'}</td><td>${usable?number(c.pctGross):'—'}</td></tr>`;
  }).join('')||'<tr><td colspan="5">Requiere dos snapshots completos y comparables.</td></tr>'}</tbody></table></div>
  <div class="panel-caption">Mismo activo, horizonte, vencimientos, contratos y fuente de referencia; máximo 15 minutos entre observaciones de la misma fecha ET. Umbral heredado ±0,5 % del DEX bruto. CALL/PUT describe el cambio del modelo, no compras/ventas ejecutadas. Las griegas no incluyen una fecha de publicación por campo: la hora indicada es de consulta. La base se conserva localmente y solo se recupera si sigue siendo válida para el mismo activo y fecha ET.</div></section>`;
}
