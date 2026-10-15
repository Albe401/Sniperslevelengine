import {reportOiHeatmap} from './option-oi-heatmap-report';
export {reportOiHeatmap} from './option-oi-heatmap-report';
import {escapeHtml as esc} from './model';
import type {EtfReport} from './etf-dealers';
import {compactOi,STATE_LABELS_ES,type Heatmap,type Cell} from './option-oi-heatmap';
import './option-oi-heatmap-panel.css';
type View='comparison'|'call'|'put';
let view:View='comparison',scale:'linear'|'log1p'='linear',normalization:'column'|'matrix'='column';
let strikeCount=31,expirationPage=0,selected='';
let cached:{report:EtfReport;key:string;map:Heatmap}|undefined;

export function optionOiHeatmapPanel(symbol:string,r?:EtfReport,now=Date.now()):string {
 let map:Heatmap|undefined,error='';
 if(r)try {
  if(r.symbol!==symbol||r.at>now||now-r.at>300_000)throw Error('Cadena caducada o de otro activo; actualiza la cadena.');
  const key=JSON.stringify([normalization,scale]);
  if(cached?.report===r&&cached.key===key)map=cached.map;
  else {map=reportOiHeatmap(r,normalization,scale);cached={report:r,key,map};}
 }catch(e){error=e instanceof Error?e.message:String(e);}
 const sides=view==='comparison'?['call','put']:[view];
 const expirations=map?.expirations??[],pages=Math.max(1,Math.ceil(expirations.length/6));expirationPage=Math.min(expirationPage,pages-1);
 const dates=expirations.slice(expirationPage*6,(expirationPage+1)*6),all=map?.strikes??[];
 const center=map?.underlying?.price??r?.spot;
 const closest=all.length&&Number.isFinite(center)?all.reduce((best,v,i)=>Math.abs(v-center!)<Math.abs(all[best]-center!)?i:best,0):Math.floor(all.length/2);
 const start=Math.max(0,Math.min(all.length-strikeCount,closest-Math.floor(strikeCount/2))),strikes=all.slice(start,start+strikeCount);
 const cells=new Map(map?.cells.map(c=>[JSON.stringify([c.strike,c.expiration,c.side]),c])??[]);
 const chosen=map?.cells.find(c=>c.id===selected);
 const number=(v:number|null|undefined)=>v==null?'Sin datos':v.toLocaleString('es');
 const title=(c:Cell)=>`${symbol} · ${c.strike} · ${c.expiration} · ${c.side.toUpperCase()} · ${STATE_LABELS_ES[c.state]} · OI ${number(c.oi)} · ${c.knownContracts}/${c.expectedContracts} contratos verificados · máximo de color ${number(c.colorMax)}`;
 const renderCell=(c:Cell|undefined)=>{if(!c)return '<td>—</td>';const heat=c.intensity==null?'':`style="background:rgb(${Math.round(240-205*c.intensity)},${Math.round(248-64*c.intensity)},${Math.round(245-110*c.intensity)});color:#06251d"`;
  return `<td><button type="button" class="oi-data-cell oi-state-${c.state}" ${heat} data-oi-cell="${esc(c.id)}" aria-pressed="${selected===c.id}" aria-label="${esc(title(c))}" title="${esc(title(c))}">${c.oi===null?'—':compactOi(c.oi)}</button></td>`;};
 const badge=error?'Datos no utilizables':map?(map.universeComplete?'Referencia verificada':'Referencia parcial'):'Esperando cadena';
 return `<section class="oi-layout" data-oi-layout aria-label="Heatmap de interés abierto">
 <header class="oi-layout-header"><div><span class="oi-layout-eyebrow">CADENA DE OPCIONES · ${esc(symbol)}</span><h3>Mapa de interés abierto</h3><p>OI diario por strike, vencimiento y lado</p></div><span class="oi-layout-badge">${badge}</span></header>
 <div class="oi-layout-toolbar"><div class="oi-layout-views" role="group" aria-label="Vista del mapa">${([['comparison','Comparación'],['call','CALL'],['put','PUT']] as const).map(([v,label])=>`<button type="button" data-oi-view="${v}" aria-pressed="${view===v}">${label}</button>`).join('')}</div>
 <label>Comparar colores<select data-oi-normalization><option value="column" ${normalization==='column'?'selected':''}>Por vencimiento y lado</option><option value="matrix" ${normalization==='matrix'?'selected':''}>Matriz completa · CALL y PUT</option></select></label>
 <label>Escala<select data-oi-scale><option value="linear" ${scale==='linear'?'selected':''}>Lineal</option><option value="log1p" ${scale==='log1p'?'selected':''}>Logarítmica</option></select></label>
 <button type="button" data-oi-strikes="-5" ${strikeCount<=11?'disabled':''}>− strikes</button><button type="button" data-oi-strikes="5" ${strikeCount>=101?'disabled':''}>+ strikes</button></div>
 <div class="oi-layout-metadata"><span>Cobertura OI <b>${map?.coverage==null?'Pendiente':(map.coverage*100).toFixed(1)+'%'}</b></span><span>Fecha efectiva OI <b>${esc(map?.oiDate??'No suministrada')}</b></span><span>Precio al consultar <b>${number(map?.underlying?.price)}</b></span><span>Color <b>${map?.scaleStatus==='provisional'?'Provisional':map?'Datos del filtro':'Pendiente'}</b></span></div>
 ${map?.scaleStatus==='provisional'?'<p class="oi-layout-warning" role="status">Escala provisional: faltan contratos o datos verificables. Los máximos observados pueden cambiar.</p>':''}
 ${map?`<div class="oi-layout-toolbar"><button type="button" data-oi-expiry-page="-1" ${expirationPage===0?'disabled':''}>Vencimientos anteriores</button><span>${expirations.length} vencimientos · página ${expirationPage+1}/${pages} · ${strikes.length}/${all.length} strikes visibles</span><button type="button" data-oi-expiry-page="1" ${expirationPage===pages-1?'disabled':''}>Siguientes vencimientos</button></div>`:''}
 <div class="oi-layout-maps ${view==='comparison'?'oi-layout-comparison':''}">${sides.map(side=>`<article class="oi-layout-map oi-layout-${side}"><header><strong>${side.toUpperCase()}</strong><span>OI · contratos</span></header>${map&&strikes.length&&dates.length?`<div class="oi-data-scroll"><table class="oi-data-table"><caption>${side.toUpperCase()} · ${normalization==='column'?'máximo independiente por columna':'máximo común de la matriz'}</caption><thead><tr><th scope="col">Strike</th>${dates.map(d=>`<th scope="col">${esc(d)}</th>`).join('')}</tr></thead><tbody>${strikes.map(strike=>`<tr ${map!.underlying?.price===strike?'class="oi-atm"':''}><th scope="row">${number(strike)}</th>${dates.map(d=>renderCell(cells.get(JSON.stringify([strike,d,side])))).join('')}</tr>`).join('')}</tbody></table></div>`:`<div class="oi-no-data"><strong>${error?'No se pudo verificar el mapa':'Esperando datos de la cadena'}</strong><p>${esc(error||'Actualiza la cadena completa. No se muestran números ni colores inventados.')}</p></div>`}<footer><span>Menor OI</span><i aria-hidden="true"></i><span>Mayor OI · ${normalization==='column'?'por columna':'matriz'}</span></footer></article>`).join('')}</div>
 <div class="oi-layout-detail" role="status"><strong>Detalle de la celda</strong><span>${chosen?esc(title(chosen)):'Selecciona una celda para ver OI exacto, vencimiento, estado y cobertura.'}</span></div>
 <p class="oi-layout-footnote">${map?`Captura ${esc(new Date(map.observedMs).toLocaleString('es',{timeZone:'America/New_York'}))} ET · ${map.knownContracts}/${map.expectedContracts} contratos estándar con OI · ${map.rejected.length} rechazos · ${map.nonStandard.length} excluidos · ${map.unknownDeliverable.length} entregables pendientes. `:''}Normalización propia; CME documenta vistas por columna y matriz, no esta ecuación de color. Los máximos usan todos los strikes del filtro. OI no identifica compras, ventas ni dirección futura.</p></section>`;
}

export function bindOptionOiHeatmap(host:HTMLElement,refresh:()=>void):void {
 const refreshKeepingScroll=()=>{
  const positions=new Map([...host.querySelectorAll<HTMLElement>('.oi-data-scroll')].map(p=>[p.closest('.oi-layout-map')?.querySelector('header strong')?.textContent,{top:p.scrollTop,left:p.scrollLeft}]));
  refresh();
  host.querySelectorAll<HTMLElement>('.oi-data-scroll').forEach(p=>{const old=positions.get(p.closest('.oi-layout-map')?.querySelector('header strong')?.textContent);if(old){p.scrollTop=old.top;p.scrollLeft=old.left;}});
 };
 host.querySelectorAll<HTMLButtonElement>('[data-oi-view]').forEach(b=>b.onclick=()=>{const v=b.dataset.oiView;if(v==='comparison'||v==='call'||v==='put'){view=v;refreshKeepingScroll();}});
 const select=host.querySelector<HTMLSelectElement>('[data-oi-scale]');if(select)select.onchange=()=>{scale=select.value==='log1p'?'log1p':'linear';refreshKeepingScroll();};
 const mode=host.querySelector<HTMLSelectElement>('[data-oi-normalization]');if(mode)mode.onchange=()=>{normalization=mode.value==='matrix'?'matrix':'column';refreshKeepingScroll();};
 host.querySelectorAll<HTMLButtonElement>('[data-oi-strikes]').forEach(b=>b.onclick=()=>{strikeCount=Math.max(11,Math.min(101,strikeCount+Number(b.dataset.oiStrikes)));refresh();});
 host.querySelectorAll<HTMLButtonElement>('[data-oi-expiry-page]').forEach(b=>b.onclick=()=>{expirationPage=Math.max(0,expirationPage+Number(b.dataset.oiExpiryPage));refresh();});
 host.querySelectorAll<HTMLButtonElement>('[data-oi-cell]').forEach(b=>b.onclick=()=>{selected=b.dataset.oiCell??'';host.querySelectorAll('[data-oi-cell]').forEach(other=>other.setAttribute('aria-pressed',String(other===b)));const detail=host.querySelector('.oi-layout-detail span');if(detail)detail.textContent=b.title;});
}
