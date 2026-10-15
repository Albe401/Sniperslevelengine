import {escapeHtml as esc,price} from './model';
export interface LegacyPosition {
  legacyId:number;symbol:string;transferredMs:number;
  trade:{openedMs?:number;side:string;entry:number;current:number;stop:number;target1:number;target2:number|null;pnlPoints:number;highest:number;lowest:number;mfe:number;mae:number;lastMs:number|null;closedMs:number|null;exitReason:string|null};
}
export function legacyPositions(positions:LegacyPosition[]|undefined,enabled:boolean,fresh:boolean,now=Date.now()):string {
  if(!positions?.length)return '';
  return `<div class="panel-caption"><strong>Posiciones traspasadas del diario Python</strong> · ${enabled&&fresh?'Seguimiento habilitado':'Seguimiento detenido'}<br>El archivo conserva el estado original; esta tabla muestra su continuación en Rust. PnL en puntos del subyacente.</div><div class="table-wrap"><table><thead><tr><th>ORIGEN / ACTIVO</th><th>ESTADO</th><th>ENTRADA / ÚLTIMO</th><th>STOP / OBJETIVOS</th><th>PnL / MFE / MAE</th></tr></thead><tbody>${positions.map(p=>{
    const t=p.trade;
    const current=fresh&&enabled&&t.lastMs!=null&&t.lastMs>p.transferredMs&&t.lastMs<=now&&now-t.lastMs<=15000;
    return `<tr><td>Python #${esc(String(p.legacyId))}<small class="exposure-dates">${esc(p.symbol)} · ${esc(t.side)}</small></td><td>${t.closedMs!=null?'Cerrada':current?'Abierta · precio vigente':'Abierta · esperando precio vigente'}${t.exitReason?`<small class="exposure-dates">${esc(t.exitReason)}</small>`:''}</td><td>${price(t.entry)} / ${price(t.current)}${!current&&t.closedMs==null?'<small class="exposure-dates">Último valor guardado</small>':''}</td><td>${price(t.stop)}<small class="exposure-dates">${price(t.target1)} / ${t.target2==null?'—':price(t.target2)}</small></td><td>${price(t.pnlPoints)}<small class="exposure-dates">${price(t.mfe)} / ${price(t.mae)}</small></td></tr>`;
  }).join('')}</tbody></table></div>`;
}
