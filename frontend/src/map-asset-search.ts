import {escapeHtml as esc} from './model';
import './map-asset-search.css';
export interface MapAsset {symbol:string;name:string;kind:string;sector?:string|null}
export interface MapAssetSearch {assets:MapAsset[];pending?:boolean}
let query='';
export function mapAssetMatches(assets:MapAsset[],text:string):MapAsset[]{
 const words=text.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
 return assets.filter(a=>a.kind!=='INDEX'&&words.every(w=>`${a.symbol} ${a.name} ${a.sector??''}`.toLocaleLowerCase().includes(w)))
  .sort((a,b)=>Number(b.symbol.toLocaleLowerCase()===text.trim().toLocaleLowerCase())-Number(a.symbol.toLocaleLowerCase()===text.trim().toLocaleLowerCase())||a.symbol.localeCompare(b.symbol));
}
function results(options:MapAssetSearch):string {
 const rows=mapAssetMatches(options.assets,query);
 return `<span role="status">${query.trim()?`${rows.length} coincidencias`:`${rows.length} activos disponibles · busca por símbolo o nombre`}</span>${query.trim()?`<div class="tm-asset-results">${rows.slice(0,12).map(a=>`<button type="button" data-map-asset="${esc(a.symbol)}" ${options.pending?'disabled':''}><b>${esc(a.symbol)}</b><span>${esc(a.name)}</span></button>`).join('')||'<span>Sin coincidencias en el catálogo disponible.</span>'}</div>${rows.length>12?'<small>Afina la búsqueda para ver más resultados.</small>':''}`:''}`;
}
export function mapAssetSearch(options:MapAssetSearch):string {
 return `<form class="tm-asset-search" data-map-asset-form><label for="map-asset-query">Buscar activo</label><div class="tm-asset-entry"><input id="map-asset-query" name="mapAsset" type="search" autocomplete="off" placeholder="Símbolo o nombre · SPY, Apple…" value="${esc(query)}" ${options.pending?'disabled':''}><button type="submit" ${options.pending?'disabled':''}>${options.pending?'Cambiando…':'Ver activo'}</button></div><div data-map-asset-results>${results(options)}</div></form>`;
}
export function bindMapAssetSearch(root:HTMLElement,options:MapAssetSearch,onSelect:(symbol:string)=>void):void {
 const form=root.querySelector<HTMLFormElement>('[data-map-asset-form]'),input=root.querySelector<HTMLInputElement>('#map-asset-query'),host=root.querySelector<HTMLElement>('[data-map-asset-results]');
 if(!form||!input||!host)return;
 const choose=(symbol:string)=>{if(options.pending)return;query='';onSelect(symbol);};
 input.oninput=()=>{query=input.value;host.innerHTML=results(options);};
 form.onclick=e=>{const button=(e.target as Element).closest<HTMLElement>('[data-map-asset]');if(button){e.preventDefault();choose(button.dataset.mapAsset!);}};
 form.onsubmit=e=>{e.preventDefault();if(options.pending)return;const rows=mapAssetMatches(options.assets,input.value),exact=rows.find(a=>a.symbol.toUpperCase()===input.value.trim().toUpperCase());if(exact||rows.length===1)choose((exact??rows[0]).symbol);else{host.innerHTML=results(options);host.querySelector('[role="status"]')!.textContent=rows.length?'Selecciona un resultado para cambiar de activo.':'Sin coincidencias en el catálogo disponible.';}};
 input.onkeydown=e=>{if(e.key==='Escape'){query='';input.value='';host.innerHTML=results(options);}if(e.key==='ArrowDown'){e.preventDefault();host.querySelector<HTMLButtonElement>('button')?.focus();}};
}
