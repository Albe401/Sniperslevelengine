import {escapeHtml as esc,price} from './model';
import {interpretationFresh} from './confirmations';
export interface OptionLiquidity {status:string;ready:boolean;contractsTracked:number;quoted:number;recent:number;depth:number;quoteCoveragePct:number;recentCoveragePct:number;depthCoveragePct:number;averageSpreadPct:number|null;entryConfirmed:boolean}
export function optionLiquidityPanel(data:OptionLiquidity|undefined,symbol:string,streaming:boolean,published:number|undefined,now=Date.now()):string {
 const fresh=streaming&&interpretationFresh(published,now);
 const pct=(v:number|undefined)=>fresh&&v!=null?`${price(v)} %`:'—';
 return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">OPCIONES ${esc(symbol)} · COTIZACIONES OBSERVADAS</span><h2>Liquidez de opciones</h2></div><span class="tag">${fresh?esc(data?.status??'Por verificar'):'Sin conexión vigente'}</span></div>
 <div class="table-wrap"><table><thead><tr><th>CONTRATOS SELECCIONADOS</th><th>COBERTURA BID/ASK</th><th>QUOTES ≤12S</th><th>PROFUNDIDAD</th><th>SPREAD MEDIO</th></tr></thead><tbody><tr><td>${data?.contractsTracked??0}</td><td>${pct(data?.quoteCoveragePct)}</td><td>${pct(data?.recentCoveragePct)}</td><td>${pct(data?.depthCoveragePct)}</td><td>${pct(data?.averageSpreadPct??undefined)}</td></tr></tbody></table></div>
 <div class="panel-caption">Filtro del activo seleccionado: mínimo dos cotizaciones, cobertura reciente ≥70 %, profundidad bid/ask ≥50 % y spread medio ≤15 %. Los contratos sin cotización permanecen en el denominador. «Liquidez aceptable» describe las cotizaciones observadas; no garantiza ejecución ni autoriza entradas.</div></section>`;
}
