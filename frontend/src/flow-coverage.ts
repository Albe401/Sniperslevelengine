import {escapeHtml as esc,price} from './model';
import {interpretationFresh} from './confirmations';
export interface ContractWindow {ticker:string;kind:string;strike:number;expiration:string;minutes:number;covered:boolean;contracts:number|null;premium:number|null;trades:number|null;reason:string}
export function flowCoveragePanel(rows:ContractWindow[]|undefined,connected:boolean,published:number|undefined,lastEvent:number|null|undefined,now=Date.now()):string {
  const fresh=connected&&interpretationFresh(published,now)&&lastEvent!=null&&lastEvent<=now&&now-lastEvent<=15_000;
  const value=(n:number|null)=>n==null?'—':price(n);
  return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">OPCIONES · CONTINUIDAD DE OBSERVACIÓN</span><h2>Cobertura de volumen por contrato</h2></div><span class="tag">BASE PARA MUROS Y OBJETIVOS</span></div>
  <details><summary>Ver ventanas de uno y cinco minutos</summary><div class="table-wrap"><table><thead><tr><th>CONTRATO</th><th>VENTANA</th><th>COBERTURA</th><th>CONTRATOS NEGOCIADOS</th><th>PRIMA USD</th></tr></thead><tbody>${(rows??[]).map(r=>{
    const valid=fresh&&r.covered;
    return `<tr><td>${esc(r.ticker)}</td><td>${r.minutes}m</td><td>${valid?'Ventana observada':'Sin cobertura vigente'}<small class="exposure-dates">${esc(r.reason)}</small></td><td>${valid?value(r.contracts):'—'}</td><td>${valid?value(r.premium):'—'}</td></tr>`;
  }).join('')||'<tr><td colspan="5">Esperando contratos suscritos y eventos válidos.</td></tr>'}</tbody></table></div></details>
  <div class="panel-caption">Una ventana empieza a acreditarse después de recibir una operación válida y observar todo su intervalo sin interrupción conocida. Al reconectar o cambiar el contrato, se vuelve a esperar. Cero significa una ventana observada sin operaciones; — indica dato desconocido. Esta cobertura por contrato todavía no confirma cobertura de las zonas de los muros ni autoriza entradas.</div></section>`;
}
