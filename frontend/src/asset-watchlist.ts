import registry from '../../fixtures/legacy-universe.json';
import {escapeHtml as esc} from './model';

export interface AssetConfig {name:string;kind:string;sector:string|null;live_range:number}
export const bundledAssets:Record<string,AssetConfig>=registry.assets;
export function searchAssets(assets:Record<string,AssetConfig>,query='') {
 const words=query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
 return Object.entries(assets).filter(([symbol,a])=>a.kind!=='INDEX'&&words.every(w=>`${symbol} ${a.name} ${a.sector??''}`.toLocaleLowerCase().includes(w)))
  .sort(([a],[b])=>a.localeCompare(b));
}
export function assetResults(assets:Record<string,AssetConfig>,query=''):string {
 const rows=searchAssets(assets,query);
 return `<p role="status">${rows.length} de ${searchAssets(assets).length} activos · acciones y ETF</p><div class="watchlist-results">${rows.map(([symbol,a])=>`<button type="button" data-watchlist-symbol="${esc(symbol)}"><strong>${esc(symbol)}</strong><span>${esc(a.name)}</span><small>${esc(a.sector??a.kind)}</small></button>`).join('')||'<p>Sin coincidencias. Busca por símbolo, nombre o sector.</p>'}</div>`;
}
