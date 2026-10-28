import {invoke,isTauri} from './snapshot-observation';
import type {EtfReport} from './etf-dealers';
import type {EtfDetail} from './etf-enrichment';
import {scopeContractReport} from './contract-profile';
import {escapeHtml as esc} from './model';
import './contract-inventory.css';
export type InventoryHorizon='DAY'|'WEEK'|'FOUR_WEEKS'|'MONTH'|'THREE_MONTHS'|'SIX_MONTHS';
export const oiHorizonChoices:[InventoryHorizon,string][]=[['DAY','0DTE'],['WEEK','1 semana'],['MONTH','1 mes'],['THREE_MONTHS','3 meses'],['SIX_MONTHS','6 meses']];
const day=(n:number)=>new Date(n).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
export function inventoryEnd(today:string,months=6):string {
 const [y,m,d]=today.split('-').map(Number),last=new Date(Date.UTC(y,m-1+months+1,0)).getUTCDate();
 return new Date(Date.UTC(y,m-1+months,Math.min(d,last))).toISOString().slice(0,10);
}
export function inventoryDates(r:EtfReport|undefined,mode:InventoryHorizon,now:number):string[]{
 const today=day(now),days=mode==='DAY'?0:mode==='WEEK'?7:28;
 const end=mode==='SIX_MONTHS'?inventoryEnd(today):mode==='THREE_MONTHS'?inventoryEnd(today,3):mode==='MONTH'?inventoryEnd(today,1):new Date(Date.parse(today+'T12:00:00Z')+days*86400000).toISOString().slice(0,10);
 const referenceDates=(r?.heatmap?.reference??[]).map(c=>(c as {expiration_date?:unknown})?.expiration_date).filter((d):d is string=>typeof d==='string');
 return [...new Set([...(r?.enrichment?.details.map(c=>c.expiration)??[]),...referenceDates].filter(d=>/^\d{4}-\d{2}-\d{2}$/.test(d)&&d>=today&&d<=end))].sort();
}
export function inventoryTotals(ds:EtfDetail[]){
 const side=(kind:string)=>{const rows=ds.filter(c=>c.kind===kind),sum=(key:'oi'|'volume')=>rows.every(c=>c[key]!=null&&Number.isFinite(c[key])&&c[key]!>=0)?rows.reduce((n,c)=>n+c[key]!,0):null;
  return {series:rows.length,oi:sum('oi'),volume:sum('volume'),oiKnown:rows.filter(c=>c.oi!=null).length};};
 return {call:side('call'),put:side('put')};
}
export function validInventory(r:EtfReport,symbol:string,now:number):boolean {
 const ds=r?.enrichment?.details,today=day(now),end=inventoryEnd(today);
 return !!ds&&r.symbol===symbol&&r.days===183&&Number.isFinite(r.at)&&r.at<=now&&now-r.at<=300000&&day(r.at)===day(now)&&r.contracts===ds.length&&ds.length<=100000&&new Set(ds.map(c=>c.ticker)).size===ds.length&&ds.every(c=>Number.isFinite(c.strike)&&c.strike>0&&['call','put'].includes(c.kind)&&/^\d{4}-\d{2}-\d{2}$/.test(c.expiration)&&c.expiration>=today&&c.expiration<=end&&[c.oi,c.volume,c.dex,c.gex].every(v=>v==null||Number.isFinite(v)&&v>=0));
}
const memory=new Map<string,{report?:EtfReport;busy:boolean;error:string;attempt:number;capturedMs?:number}>();
/** Populate the existing reader from a saved frame; never start a provider request. */
export function seedCapturedInventory(report:EtfReport|undefined,symbol:string,capturedMs:number){
 memory.delete(symbol);scoped=undefined;
 if(report&&report.at<=capturedMs&&day(report.at)===day(capturedMs)&&validInventory(report,symbol,report.at))memory.set(symbol,{report,busy:false,error:'',attempt:capturedMs,capturedMs});
 while(memory.size>4)memory.delete(memory.keys().next().value!);
}
let horizon:InventoryHorizon='SIX_MONTHS',expiry='',page=0,search='',sideFilter='call';
export function inventoryOiHorizonControls():string {return `<div class="saved-chain-toolbar oi-horizon-controls" role="group" aria-label="Plazo del interés abierto">${oiHorizonChoices.map(([mode,label])=>`<button type="button" data-oi-inventory-horizon="${mode}" aria-pressed="${horizon===mode}">${label}</button>`).join('')}</div>`;}
export function selectInventoryOiHorizon(mode:InventoryHorizon):void {horizon=mode;expiry='';page=0;scoped=undefined;}
let scoped:{report:EtfReport;key:string;value:EtfReport}|undefined;
export function inventoryProfile(symbol:string,now:number):EtfReport|undefined {
 const entry=memory.get(symbol),r=entry?.report;if(!r||r.symbol!==symbol||r.at>now||entry?.capturedMs!==now&&now-r.at>300000||day(r.at)!==day(now))return;
 const key=symbol+':'+day(now)+':'+horizon+':'+expiry;
 if(scoped?.report===r&&scoped.key===key)return scoped.value;
 const dates=inventoryDates(r,horizon,now).filter(d=>!expiry||d===expiry);
 const value=scopeContractReport(r,dates);scoped={report:r,key,value};return value;
}
let panelCache:{report:EtfReport|undefined;key:string;html:string}|undefined;
/** Compact controller for the heatmap; retains the existing fetch/cache/filter lifecycle. */
export function inventoryHeatmapControls(symbol:string,now=Date.now()):string {
 const entry=memory.get(symbol);
 return `<section class="inventory-heatmap-controls" data-contract-inventory aria-label="Filtros de la cadena"><div class="inventory-tools"><button data-inventory-refresh ${entry?.busy?'disabled':''}>${entry?.busy?'Consultando cadena…':'Actualizar cadena'}</button>${([['DAY','0DTE'],['WEEK','Semanal · 7 días'],['FOUR_WEEKS','4 semanas'],['MONTH','Mensual · un mes'],['SIX_MONTHS','Todos · 6 meses']] as const).map(([mode,label])=>`<button data-inventory-horizon="${mode}" aria-pressed="${horizon===mode}">${label}</button>`).join('')}<label>Vencimiento <select data-inventory-expiry><option value="">Todas del horizonte</option>${inventoryDates(entry?.report,horizon,now).map(d=>`<option ${d===expiry?'selected':''} value="${d}">${d}</option>`).join('')}</select></label></div><p class="inventory-status" role="status">${esc(entry?.error||(entry?.busy?'Consultando los datos del heatmap.':''))}</p></section>`;
}
export function inventoryPanel(symbol:string,now=Date.now()):string {
 const entry=memory.get(symbol),r=inventoryProfile(symbol,now);
 const panelKey=JSON.stringify([symbol,horizon,expiry,page,search,sideFilter,entry?.busy,entry?.error,!!r,day(now)]);
 if(panelCache&&panelCache.report===r&&panelCache.key===panelKey)return panelCache.html;
 const totals=inventoryTotals(r?.enrichment?.details??[]),n=(v:number|null)=>v==null?'Sin datos':v.toLocaleString('es');
 const allDetails=r?.enrichment?.details??[],details=allDetails.filter(c=>(sideFilter==='all'||c.kind===sideFilter)&&(!search||c.ticker.toUpperCase().includes(search.toUpperCase())||String(c.strike).includes(search))),pages=Math.max(1,Math.ceil(details.length/100)),current=Math.min(page,pages-1),visible=details.slice(current*100,(current+1)*100);
 const row=(label:string,ds:EtfDetail[])=>{const t=inventoryTotals(ds);return `<tr><th>${esc(label)}</th><td>${n(t.call.series)}</td><td>${n(t.put.series)}</td><td>${n(t.call.oi)}</td><td>${n(t.put.oi)}</td><td>${n(t.call.volume)}</td><td>${n(t.put.volume)}</td></tr>`;};
 const html=`<section class="contract-inventory" data-contract-inventory><header><div class="inventory-title"><span>SNIPERS · CADENA DE OPCIONES</span><strong>${esc(symbol)} <em>CALL / PUT</em></strong><small>Todos los strikes · expiraciones hasta seis meses</small></div><button data-inventory-refresh ${entry?.busy?'disabled':''}>${entry?.busy?'Consultando todas las expiraciones…':'Actualizar cadena completa'}</button></header><nav class="inventory-side-tabs" aria-label="Contratos por lado">${(['call','put'] as const).map(kind=>`<button data-inventory-side-tab="${kind}" aria-pressed="${sideFilter===kind}"><strong>${kind.toUpperCase()}</strong><span>${r?totals[kind].series.toLocaleString('es'):'—'} series · OI ${r?n(totals[kind].oi):'—'}</span></button>`).join('')}</nav><div class="inventory-tools" aria-label="Buscar contratos">${([['DAY','0DTE'],['WEEK','Semanal · 7 días'],['FOUR_WEEKS','4 semanas'],['MONTH','Mensual · un mes'],['SIX_MONTHS','Todos · 6 meses']] as const).map(([mode,label])=>`<button data-inventory-horizon="${mode}" aria-pressed="${horizon===mode}">${label}</button>`).join('')}<label>Expiración <select data-inventory-expiry><option value="">Todas del horizonte</option>${inventoryDates(entry?.report,horizon,now).map(d=>`<option ${d===expiry?'selected':''} value="${d}">${d}</option>`).join('')}</select></label><label>Buscar strike o contrato <input data-inventory-search value="${esc(search)}" placeholder="Ej. 750 o QQQ" /></label><button data-inventory-find>Buscar</button><button data-inventory-clear>Limpiar</button></div><p class="inventory-status" role="status">${esc(entry?.error|| (entry?.busy?'Consulta paginada de todos los strikes; puede tardar.':r?`Snapshot ${new Date(r.at).toLocaleString('es',{timeZone:'America/New_York'})} ET · ${r.expirations.length} expiraciones · ${r.contracts} series. Rango hasta ${inventoryEnd(day(now))}.`:'Sin cadena completa vigente. Abre la aplicación y consulta; los errores del proveedor se muestran aquí.'))}</p>${r?`<div class="inventory-cards">${(['call','put'] as const).map(kind=>`<article class="inventory-side inventory-${kind}"><header>${kind.toUpperCase()} <span>INTERÉS ABIERTO</span></header><strong>OI ${n(totals[kind].oi)}</strong><div><span>Series distintas <b>${n(totals[kind].series)}</b></span><span>Volumen negociado <b>${n(totals[kind].volume)}</b></span></div><small>OI disponible en ${totals[kind].oiKnown} / ${totals[kind].series} series</small></article>`).join('')}</div><details class="inventory-expirations" data-horizon-details="inventory-expirations"><summary>Expiraciones y cantidades <span>${r.expirations.length} vencimientos</span></summary><div class="inventory-table"><table><thead><tr><th>Expiración</th><th>Series CALL</th><th>Series PUT</th><th>OI CALL</th><th>OI PUT</th><th>Vol. CALL</th><th>Vol. PUT</th></tr></thead><tbody>${row('TOTAL DEL FILTRO',r.enrichment!.details)}${r.expirations.map(d=>row(d,r.enrichment!.details.filter(c=>c.expiration===d))).join('')}</tbody></table></div></details><details open class="inventory-contracts" data-horizon-details="inventory-contracts"><summary>Contratos ${sideFilter.toUpperCase()} · ${details.length} series</summary><div><button data-inventory-page="-1" ${current===0?'disabled':''}>Anterior</button> Página ${current+1} / ${pages} · 100 contratos por página <button data-inventory-page="1" ${current===pages-1?'disabled':''}>Siguiente</button></div><div class="inventory-table"><table><thead><tr><th>Contrato</th><th>Expiración</th><th>Lado</th><th>Strike</th><th>OI</th><th>Volumen</th></tr></thead><tbody>${visible.map(c=>`<tr><th>${esc(c.ticker)}</th><td>${esc(c.expiration)}</td><td>${esc(c.kind.toUpperCase())}</td><td>${n(c.strike)}</td><td>${n(c.oi)}</td><td>${n(c.volume)}</td></tr>`).join('')}</tbody></table></div></details>`:''}<small>Series = contratos distintos por vencimiento, strike y lado. OI = posiciones abiertas del snapshot; volumen = contratos negociados. Datos ausentes conservan «Sin datos». Más CALL u OI CALL no implica compras alcistas: cada contrato tiene comprador y vendedor. Horizontes acumulados; no se suman entre sí.</small></section>`;
 panelCache={report:r,key:panelKey,html};return html;
}
export async function requestInventory(symbol:string,refresh:()=>void,force=false){
 if(!isTauri())return;
 const previous=memory.get(symbol),now=Date.now();if(previous?.busy||!force&&previous&&now-previous.attempt<300000)return;
 const entry={...previous,busy:true,error:'',attempt:now};memory.set(symbol,entry);while(memory.size>4)memory.delete(memory.keys().next().value!);refresh();
 try{const r=await invoke<EtfReport>('get_contract_inventory',{symbol});if(!validInventory(r,symbol,Date.now()))throw Error('Cadena inválida, caducada o fuera del rango de seis meses.');entry.report=r;}
 catch(e){entry.report=undefined;entry.error=String(e);}
 finally{entry.busy=false;refresh();}
}
export function bindInventory(symbol:string,refresh:()=>void){
 const root=document.querySelector<HTMLElement>('[data-contract-inventory]');if(!root)return;
 root.querySelectorAll<HTMLButtonElement>('[data-inventory-side-tab]').forEach(b=>b.onclick=()=>{sideFilter=b.dataset.inventorySideTab!;page=0;refresh();});
 root.querySelector<HTMLButtonElement>('[data-inventory-refresh]')!.onclick=()=>{void requestInventory(symbol,refresh,true);};
 root.querySelectorAll<HTMLButtonElement>('[data-inventory-horizon]').forEach(b=>b.onclick=()=>{horizon=b.dataset.inventoryHorizon as InventoryHorizon;expiry='';page=0;refresh();});
 root.querySelector<HTMLSelectElement>('[data-inventory-expiry]')!.onchange=e=>{expiry=(e.target as HTMLSelectElement).value;page=0;refresh();};
 const searchInput=root.querySelector<HTMLInputElement>('[data-inventory-search]'),side=root.querySelector<HTMLSelectElement>('[data-inventory-side]');
 const find=root.querySelector<HTMLButtonElement>('[data-inventory-find]');if(find)find.onclick=()=>{search=searchInput?.value.trim()??'';sideFilter=side?.value??sideFilter;page=0;refresh();};
 if(searchInput)searchInput.onkeydown=e=>{if(e.key==='Enter')find?.click();};
 const clear=root.querySelector<HTMLButtonElement>('[data-inventory-clear]');if(clear)clear.onclick=()=>{search='';expiry='';page=0;refresh();};
 root.querySelectorAll<HTMLButtonElement>('[data-inventory-page]').forEach(b=>b.onclick=()=>{page=Math.max(0,page+Number(b.dataset.inventoryPage));refresh();});
 void requestInventory(symbol,refresh);
}
