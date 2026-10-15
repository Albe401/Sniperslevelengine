import { escapeHtml as esc, price } from './model';
export interface MagnetConfirmation {
  status:string;reason:string;confirmationScore:number;alignment:string;
  primaryMagnet:number|null;offMagnet:number|null;distance:number|null;
  distancePct:number|null;entryConfirmed:boolean;
}
export const interpretationFresh=(publishedMs:number|undefined,now=Date.now())=>
  publishedMs!=null&&publishedMs<=now&&now-publishedMs<=3000;
export function confirmationPanel(rows:Record<string,MagnetConfirmation>|undefined,connected:boolean,publishedMs:number|undefined,now=Date.now()):string {
  const fresh=connected&&interpretationFresh(publishedMs,now);
  const labels:Record<string,string>={WAITING:'Esperando evidencia',NO_OPTIONS_MAGNET:'Sin imán de opciones',NO_DARK_POOL_MAGNET:'Sin imán fuera de bolsa',DIVERGENCIA:'Divergencia',PARTIAL_CONFIRMATION:'Confirmación parcial',WEAK_CONFIRMATION:'Confirmación débil',CONFIRMED:'Confirmado',CONFIRMED_STRONG:'Confirmación fuerte'};
  const value=(n:number|null)=>n==null?'—':price(n);
  return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">OPCIONES + FUERA DE BOLSA</span><h2>Confirmación del imán</h2></div><span class="tag">SIN PERMISO DE ENTRADA</span></div>
  <div class="table-wrap"><table><thead><tr><th>HORIZONTE</th><th>IMÁN PRINCIPAL</th><th>IMÁN TRF</th><th>CONFIRMACIÓN</th><th>PUNTUACIÓN</th></tr></thead><tbody>${Object.entries(rows??{}).map(([mode,c])=>{
    const certified=fresh&&['CONFIRMED','CONFIRMED_STRONG','PARTIAL_CONFIRMATION','WEAK_CONFIRMATION','DIVERGENCIA'].includes(c.status);
    return `<tr><td>${esc(mode)}</td><td>${value(c.primaryMagnet)}</td><td>${certified?value(c.offMagnet):'—'}</td><td>${fresh?esc(labels[c.status]??c.status):'Sin conexión vigente'}<small class="exposure-dates">${esc(c.reason)}</small></td><td>${certified?`${value(c.confirmationScore)} / 100`:'—'}</td></tr>`;
  }).join('')||'<tr><td colspan="5">Esperando estructura de opciones y actividad fuera de bolsa.</td></tr>'}</tbody></table></div>
  <div class="panel-caption">El imán principal sigue siendo M1 de opciones. La cercanía al imán fuera de bolsa aporta contexto; una divergencia nunca se convierte en confirmación plena por volumen. M1 sin puntuación propia conserva la referencia neutra de 50/100 del modelo anterior. Se exige cobertura conciliada, spot y print recientes (15 s), y snapshot de opciones de hasta 15 minutos. Ninguna puntuación activa operaciones.</div></section>`;
}
