import {escapeHtml as esc,price} from './model';
export interface OptionsContext {symbol:string;phase:string;detail:string;observedMs:number|null;referenceMs:number|null;contracts:number;entryConfirmed:boolean;horizons:{mode:string;observedMs:number;exposure:{gexComplete:boolean;netGex:number};structure:{complete:boolean;putWall:{strike:number}|null;callWall:{strike:number}|null}}[]}
const date=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York',year:'numeric',month:'2-digit',day:'2-digit'});
export function scannerOptionsPanel(rows:Record<string,OptionsContext>|undefined,fresh:boolean,now:number):string {
 const val=(n:number|undefined)=>n==null||!Number.isFinite(n)?'—':price(n);
 return `<details><summary>Contexto de opciones · Top 5 fijo</summary><div class="panel-caption">Cadena y exposición separadas por activo. Snapshot estructural observado; OI del último día negociado. No confirma flujo ni liquidez en vivo.</div><div class="table-wrap"><table><thead><tr><th>ACTIVO</th><th>CADENA</th><th>HORIZONTE</th><th>GEX NETO ESTIMADO</th><th>PUT WALL / CALL WALL</th></tr></thead><tbody>${Object.entries(rows??{}).map(([symbol,r])=>{
 const valid=fresh&&symbol===r.symbol&&r.phase==='ready'&&r.observedMs!=null&&Number.isFinite(r.observedMs)&&r.observedMs>0&&r.observedMs<=now&&now-r.observedMs<=900_000&&date.format(r.observedMs)===date.format(now);
 const h=valid?r.horizons.find(h=>(h.mode==='DAY'||h.mode==='NEXT_SESSION')&&h.observedMs===r.observedMs):undefined;
 return `<tr><td>${esc(symbol)}</td><td>${valid?`${r.contracts} contratos`:'Esperando contexto'}<small class="exposure-dates">${fresh?esc(r.detail):'Sin conexión vigente'}</small></td><td>${h?esc(h.mode==='DAY'?'Hoy':'Próximo vencimiento'):'—'}</td><td>${h?.exposure.gexComplete?val(h.exposure.netGex):'—'}</td><td>${h?.structure.complete?`${val(h.structure.putWall?.strike)} / ${val(h.structure.callWall?.strike)}`:'Estructura incompleta'}</td></tr>`;
 }).join('')||'<tr><td colspan="5">Esperando la selección fija de las 09:45 ET.</td></tr>'}</tbody></table></div></details>`;
}
