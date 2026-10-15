import {inventoryHeatmapControls,bindInventory,inventoryProfile} from './contract-inventory';
import {escapeHtml as esc} from './model';
import {optionOiHeatmapPanel,bindOptionOiHeatmap} from './option-oi-heatmap-panel';
let symbol='QQQ';
let heatmapExpiryTimer:ReturnType<typeof setTimeout>|undefined;
export function contractChainPanel():string {
 return `<section class="investment-contract-chain"><form data-chain-symbol class="op-controls"><label>Activo<input name="symbol" value="${esc(symbol)}" placeholder="QQQ, SPY, AAPL…" maxlength="10" required></label><button type="submit">Buscar cadena de opciones</button></form><p data-chain-error role="status"></p><div data-chain-content>${inventoryHeatmapControls(symbol)}</div><div data-chain-heatmap>${optionOiHeatmapPanel(symbol)}</div></section>`;
}
export function bindContractChain():void {
 const host=document.querySelector<HTMLElement>('.investment-radar[data-ir-active="contract-chain"] .investment-contract-chain');if(!host)return;
 const renderHeatmap=()=>{if(heatmapExpiryTimer)clearTimeout(heatmapExpiryTimer);if(!host.isConnected)return;const now=Date.now(),report=inventoryProfile(symbol,now),panel=host.querySelector<HTMLElement>('[data-chain-heatmap]')!;panel.innerHTML=optionOiHeatmapPanel(symbol,report,now);bindOptionOiHeatmap(panel,renderHeatmap);if(report)heatmapExpiryTimer=setTimeout(renderHeatmap,Math.max(1,report.at+300001-now));};
 const render=()=>{if(!host.isConnected)return;const content=host.querySelector<HTMLElement>('[data-chain-content]')!;content.innerHTML=inventoryHeatmapControls(symbol);renderHeatmap();bindInventory(symbol,render);};
 host.querySelector<HTMLFormElement>('[data-chain-symbol]')!.onsubmit=e=>{e.preventDefault();const next=String(new FormData(e.currentTarget as HTMLFormElement).get('symbol')??'').trim().toUpperCase();const error=host.querySelector<HTMLElement>('[data-chain-error]')!;if(!/^[A-Z0-9.-]{1,10}$/.test(next)){error.textContent='Escribe un símbolo válido';return;}error.textContent='';symbol=next;renderHeatmap();render();};
 renderHeatmap();
 render();
}
