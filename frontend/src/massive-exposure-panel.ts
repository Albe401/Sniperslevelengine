import type {LiveState} from './live';
import {invoke,isTauri} from './snapshot-observation';
import {GEX_WINDOWS,selectedGexWindow,selectGexWindow,rememberGexInventory} from './gex-windows';
import {escapeContextHtml} from './structural-context-html';
import {escapeHtml as esc} from './model';
import './massive-exposure-panel.css';
import {readMassiveExposure,massiveExposureIdentity} from './massive-exposure';
export {readMassiveExposure} from './massive-exposure';
export type {MassiveExposureRow,MassiveExposureView} from './massive-exposure';
const dateFormat=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'}),clockFormat=new Intl.DateTimeFormat('es-ES',{timeZone:'America/New_York',day:'2-digit',month:'2-digit',hour:'2-digit',minute:'2-digit'});
const compact=new Intl.NumberFormat('es-ES',{notation:'compact',maximumFractionDigits:2}),full=new Intl.NumberFormat('es-ES',{maximumFractionDigits:2});
// Retain navigation only; never retain market data or request another chain.
const viewports=new Map<string,{top:number;left:number}>();
const rowHtmlCache=new WeakMap<object,string>();
export function massiveExposurePanel(s:LiveState|undefined,symbol:string,active:boolean,now:number,standalone=false):string {
 const rootClass=standalone?'structural-map-preview massive-exposure-panel massive-exposure-standalone':'massive-exposure-panel';
 // Keep the nine-window source contract for saved captures; present one calendar-month choice.
 if(selectedGexWindow(symbol)==='WEEKS_4')selectGexWindow(symbol,'MONTH_1');
 const selector=`<select class="massive-gex-window" data-massive-gex-window data-gex-symbol="${esc(symbol)}" aria-label="Plazo acumulado GEX" title="Vencimientos acumulados desde la fecha de captura; meses naturales">${GEX_WINDOWS.filter(([mode])=>mode!=='WEEKS_4').map(([m,label])=>`<option value="${m}" ${selectedGexWindow(symbol)===m?'selected':''}>${label}</option>`).join('')}</select>`;
 const view=readMassiveExposure(s,symbol,active,now),title='<strong>EXPOSICIÓN GEX · MASSIVE</strong>';
 if(view.reason)return `<section class="${rootClass}" data-massive-exposure aria-label="Exposición gamma propia por strike"><header>${title}${selector}</header>${view.at?`<p class="massive-exposure-source">${esc(symbol)} · ${esc(view.start??view.expiry)} → ${esc(view.end??view.expiry)} · captura ${esc(clockFormat.format(view.at))} ET${view.historical?' · HISTÓRICO':''}</p>`:''}<p class="massive-exposure-empty">${esc(view.reason)}. Sin datos no equivale a exposición cero.</p></section>`;
 const verifiedNet=(r:import('./massive-exposure').MassiveExposureRow)=>r.callComplete!==false&&r.putComplete!==false;
 const key=JSON.stringify([symbol,view.mode,view.expiry,dateFormat.format(view.at)]),peak=view.rows.reduce((n,r)=>verifiedNet(r)?Math.max(n,Math.abs(r.netGex)):n,0);
 const reference=view.spot??s?.optionSnapshotStatus?.horizons?.find(h=>h?.mode===view.mode)?.exposure?.spot;
 const spot=typeof reference==='number'&&Number.isFinite(reference)&&reference>0?reference:null;
 const verified=view.rows.filter(verifiedNet),positive=verified.filter(r=>r.netGex>0).length,negative=verified.filter(r=>r.netGex<0).length,zero=verified.length-positive-negative,unverified=view.rows.length-verified.length;
 const zeroOiOnly=view.eligible===0&&verified.length===view.rows.length&&view.rows.length>0;
 const value=(n:number|undefined)=>n==null?'—':compact.format(n),cell=(n:number|undefined,side='',verified=true)=>`<td class="${side}" title="${!verified?'OI, gamma o multiplicador sin verificar en contratos recibidos':n==null?'No suministrado':esc(full.format(n)+' USD por 1%')}">${verified?value(n):'—'}</td>`;
 let rowsHtml=rowHtmlCache.get(view.rows);if(rowsHtml===undefined){rowsHtml=view.rows.map(row=>{const ready=verifiedNet(row),positive=row.netGex>0,width=ready&&peak?50*Math.abs(row.netGex)/peak:0;return `<tr data-massive-exposure-strike="${row.strike}" data-massive-gex="${ready?row.netGex:''}" data-massive-gex-coverage="${ready?'observed':'partial'}"><th>${full.format(row.strike)}</th>${cell(row.callGex,'massive-gex-call',row.callComplete!==false)}${cell(row.putGex,'massive-gex-put',row.putComplete!==false)}${cell(row.netGex,positive?'massive-gex-call':row.netGex<0?'massive-gex-put':'',ready)}<td><div class="massive-exposure-bar">${ready?`<i class="${positive?'positive':'negative'}" style="left:${positive?50:50-width}%;width:${width}%"></i>`:'<span title="Perfil neto sin cantidades completas">—</span>'}</div></td></tr>`;}).join('');rowHtmlCache.set(view.rows,rowsHtml);}
 return `<section class="${rootClass}" data-massive-exposure data-options-context-id="${escapeContextHtml(massiveExposureIdentity(view))}" data-exposure-key="${esc(key)}" data-exposure-page-index="all" data-exposure-reference="${spot??''}" aria-label="Exposición gamma propia por strike"><header>${title}<span>${view.complete?'CADENA RECIBIDA COMPLETA':'SUBTOTAL PARCIAL'}${view.historical?' · HISTÓRICO':''}</span></header><p class="massive-exposure-source">${esc(symbol)} · ${view.label?esc(view.label)+' · '+esc(view.start??'')+' → '+esc(view.end??''):view.mode==='DAY'?'HOY / DAY · '+esc(view.expiry):'PRÓXIMA SESIÓN · '+esc(view.expiry)}${view.expirations?' · '+view.expirations.length+' vencimientos':''} · captura ${esc(clockFormat.format(view.at))} ET · ${view.gammaUsed}/${view.eligible} contratos con gamma.${zeroOiOnly?' OI recibido conocido en cero; no requiere gamma para esas contribuciones.':''}${view.expected!=null?' Universo: '+view.expected+' series esperadas; '+view.missingContracts+' ausentes; '+view.unexpectedContracts+' adicionales.':''}</p><div class="massive-exposure-summary"><div class="massive-exposure-net"><span>Neto observado <b>${value(view.net)} USD / 1%</b></span>${selector}</div><span>${view.rows.length} strikes · + ${positive} / − ${negative} / 0 ${zero}${unverified?' · '+unverified+' sin verificar':''}</span></div><div class="massive-exposure-navigation"><span>Todos los strikes · ${full.format(view.rows[0].strike)}–${full.format(view.rows.at(-1)!.strike)} · rueda ↑↓</span><button type="button" data-massive-exposure-center ${spot==null?'disabled':''} title="Volver al strike más cercano al precio de referencia de esta captura">Al precio</button></div><div class="massive-exposure-table" tabindex="0" aria-label="Todos los strikes recibidos del plazo seleccionado"><table><thead><tr><th>STRIKE</th><th>GEX CALL</th><th>GEX PUT</th><th>GEX OI NETO</th><th>PERFIL NETO</th></tr></thead><tbody>${rowsHtml}</tbody></table></div></section>`;
}
const attempts=new Map<string,number>(),pending=new Set<string>();
export function bindMassiveExposurePanel(refresh:()=>void){
 document.querySelectorAll<HTMLSelectElement>('[data-massive-gex-window]').forEach(select=>{
  select.onchange=()=>{if(select.isConnected&&selectGexWindow(select.dataset.gexSymbol??'',select.value)){select.blur();refresh();}};
  // Share the existing command and backend cache. No request on expiry change.
  const symbol=select.dataset.gexSymbol;if(!symbol||!isTauri()||pending.has(symbol)||Date.now()-(attempts.get(symbol)??0)<300000)return;
  attempts.set(symbol,Date.now());pending.add(symbol);if(attempts.size>96)attempts.delete(attempts.keys().next().value!);
  void invoke('get_contract_inventory',{symbol}).then(value=>{rememberGexInventory(value);}).catch(()=>{}).finally(()=>{pending.delete(symbol);refresh();});
 });
 document.querySelectorAll<HTMLElement>('[data-exposure-key]').forEach(panel=>{
  const key=panel.dataset.exposureKey,host=panel.querySelector<HTMLElement>('.massive-exposure-table');if(!key||!host)return;
  const remember=()=>{viewports.set(key,{top:host.scrollTop,left:host.scrollLeft});if(viewports.size>32)viewports.delete(viewports.keys().next().value!);};
  const center=()=>{
   const spot=Number(panel.dataset.exposureReference);if(!Number.isFinite(spot)||spot<=0)return;
   let nearest:HTMLElement|undefined,distance=Infinity;
   for(const row of host.querySelectorAll<HTMLElement>('[data-massive-exposure-strike]')){const d=Math.abs(Number(row.dataset.massiveExposureStrike)-spot);if(d<distance){distance=d;nearest=row;}}
   if(nearest){const header=host.querySelector('thead')?.getBoundingClientRect().height??0;host.scrollTop+=nearest.getBoundingClientRect().top-host.getBoundingClientRect().top-(host.clientHeight+header-nearest.getBoundingClientRect().height)/2;remember();}
  };
  const previous=viewports.get(key);if(previous){host.scrollTop=previous.top;host.scrollLeft=previous.left;}else{center();remember();}
  host.onscroll=remember;
  host.onkeydown=event=>{if((event.key==='Home'||event.key==='End')&&!event.ctrlKey&&!event.metaKey&&!event.altKey){event.preventDefault();host.scrollTop=event.key==='Home'?0:host.scrollHeight;remember();}};
  const button=panel.querySelector<HTMLButtonElement>('[data-massive-exposure-center]');if(button)button.onclick=()=>{if(button.isConnected&&!button.disabled)center();};
 });
}
