import {savedWeeklyRanges} from './saved-weekly-ranges';
import {webAutoRefreshAllowed,webRefreshLabel} from './web-refresh-policy';
import {currentMapHorizon} from './reading-display';
import {blockScannerPanel,type CapturedBlockScanner} from './block-scanner-panel';

import {quickReadingPanels} from './quick-reading-panels';
import {bindLevelCardSpacing} from './level-card-spacing';
import {buildIntradayControl} from './intraday-control';
import {bindGammaLive,setCapturedProfileClock} from './gamma-live';
import {optionOiHeatmapPanel,bindOptionOiHeatmap} from './option-oi-heatmap-panel';
import {scopeContractReport} from './contract-profile';
import {seedCapturedInventory} from './contract-inventory';
import {seedCapturedAnalystTargets} from './analyst-targets';
import {seedCapturedIvHistory} from './iv-history';
import {seedCapturedComponentRead,type ComponentRead} from './qqq-components';
import type {TargetRow} from './analyst-targets-cache';
import type {EtfReport} from './etf-dealers';
import type {LiveState} from './live';
import {escapeHtml as esc} from './model';
import './day-map.css';
import watchlist from './web-watchlist.json';
import './styles.css';import './restrained.css';import './visual-readings.css';import './brand.css';import './route-design.css';import './reading-tabs.css';import './compact-reading-height.css';import '@fontsource-variable/inter';import './web.css';
const symbols=['AAPL','MSFT','AMZN','GOOGL','META','NVDA','TSLA','QQQ','SPY','IWM'];
type Extra={command:string;args?:{symbol?:string};observedMs:number;value:unknown};
type Saved={available:boolean;id:string;capturedMs:number;frame:LiveState;extras:Record<string,Extra>;coverage:Record<string,unknown>};
let selected='QQQ',view:'detailed'|'chain'|'blocks'|'weekly'='detailed',generation=0,saved:Saved|undefined,expiry='',blockQuery='',blockSide='all';
const app=document.querySelector<HTMLDivElement>('#app')!;
app.innerHTML=`<div class="web-shell"><aside class="web-sidebar"><img src="/assets/route-lion.png" alt="Snipers"><h2>SNIPERS</h2><button data-view="detailed">Lectura detallada</button><button data-view="blocks">Bloques de compra y venta</button><button data-view="chain">Radar de inversión<br><small>Cadena de opciones</small></button><button data-view="weekly">Radar de inversión<br><small>Radar semanal de precios</small></button><small>Datos guardados por el escritorio<br>Actualización lunes a viernes · cada cinco minutos ET</small></aside><main class="web-main"><header class="web-topbar"><strong>Algoritmo de probabilidad</strong><span id="web-status">Leyendo la base de datos…</span></header><nav class="web-assets">${symbols.map(s=>`<button data-symbol="${s}">${s}</button>`).join('')}</nav><section id="web-content"></section></main></div>`;
const content=document.querySelector<HTMLElement>('#web-content')!;
const search=document.createElement('div');search.className='web-ticker-search';search.innerHTML=`<label for="ticker-search">Buscar ticker del watchlist (${watchlist.length})</label><input id="ticker-search" type="search" placeholder="Escribe un ticker…" autocomplete="off"><select id="ticker-select" aria-label="Seleccionar ticker">${watchlist.map(a=>`<option value="${esc(a.symbol)}" ${a.symbol===selected?'selected':''}>${esc(a.symbol)} · ${esc(a.name)}</option>`).join('')}</select><small id="ticker-results">${watchlist.length} tickers</small>`;document.querySelector('.web-assets')!.before(search);
const searchInput=document.querySelector<HTMLInputElement>('#ticker-search')!,tickerSelect=document.querySelector<HTMLSelectElement>('#ticker-select')!;
searchInput.oninput=()=>{const q=searchInput.value.trim().toUpperCase(),rows=watchlist.filter(a=>a.symbol.includes(q));tickerSelect.innerHTML=rows.map(a=>`<option value="${esc(a.symbol)}">${esc(a.symbol)} · ${esc(a.name)}</option>`).join('');document.querySelector('#ticker-results')!.textContent=`${rows.length} tickers`;};
const selectTicker=(symbol:string)=>{selected=symbol;searchInput.value='';tickerSelect.innerHTML=watchlist.map(a=>`<option value="${esc(a.symbol)}" ${a.symbol===selected?'selected':''}>${esc(a.symbol)} · ${esc(a.name)}</option>`).join('');document.querySelector('#ticker-results')!.textContent=`${watchlist.length} tickers`;saved=undefined;expiry='',blockQuery='',blockSide='all';content.innerHTML='<section class="panel">Cargando datos guardados…</section>';void pull();};
tickerSelect.onchange=()=>selectTicker(tickerSelect.value);searchInput.onkeydown=e=>{if(e.key==='Enter'&&tickerSelect.value)selectTicker(tickerSelect.value)};
function paint(){
 if(!saved?.available)return;
 const data=saved,frame=data.frame,now=data.capturedMs;
 const clock=Date.now(),levelsCurrent=now<=clock+1000&&clock-now<=15*60_000&&!!currentMapHorizon(frame.optionSnapshotStatus,clock);
 setCapturedProfileClock(now);
 const fields=frame as LiveState&{ivHistory?:{observation:Extra};qqqReport?:ComponentRead};
 const iv=fields.ivHistory?.observation;
 const inventory=(frame as LiveState&{contractChain?:{observation:Extra}}).contractChain?.observation;
 seedCapturedInventory(inventory&&inventory.observedMs<=now?inventory.value as EtfReport:undefined,selected,now);
 seedCapturedIvHistory(iv&&iv.observedMs<=now?iv.value:undefined,selected,now);
 seedCapturedComponentRead(frame.qqq_report??fields.qqqReport,now);
 const targets=Object.values(data.extras??{}).filter(r=>r.command==='get_fmp_targets'&&r.observedMs<=now).sort((a,b)=>b.observedMs-a.observedMs)[0];
 if(targets&&(targets.value as {rows?:unknown})?.rows)seedCapturedAnalystTargets((targets.value as {rows:Record<string,TargetRow>}).rows,now);
 document.querySelector<HTMLElement>('#web-status')!.textContent=`Captura ${new Date(now).toLocaleString('es',{timeZone:'America/New_York'})} ET · ${levelsCurrent?webRefreshLabel():'niveles vigentes pendientes'}`;
 if(view==='detailed'){
  if(!levelsCurrent){
   content.innerHTML=`<section class="panel"><h2>Niveles vigentes pendientes · ${esc(selected)}</h2><p>La última captura o la cadena de opciones ya no está vigente. Captura disponible: ${new Date(now).toLocaleString('es',{timeZone:'America/New_York'})} ET. Se comprobará de nuevo automáticamente.</p></section>`;
  }else{
   content.innerHTML=`<section id="live-workspace" class="visual-reading" data-live-view="detailed">${quickReadingPanels(frame,selected,true,buildIntradayControl(frame,selected,true,now),now)}</section><p class="web-note">Lectura detallada del lote guardado en Neon. Actualización lunes a viernes · cada cinco minutos ET; sin consultas al mercado desde la web.</p>`;
   bindLevelCardSpacing();bindGammaLive(frame,selected,true,paint,now);
  }
 }else if(view==='blocks'){
  const captured=Object.values(data.extras??{}).filter(r=>r.command==='get_block_alerts'&&r.observedMs<=now).sort((a,b)=>b.observedMs-a.observedMs)[0];
  const scanner=captured?.value as CapturedBlockScanner|undefined;
  content.innerHTML=blockScannerPanel(scanner,now,Date.now(),blockQuery,blockSide);
  const filter=content.querySelector<HTMLFormElement>('[data-block-search]');
  if(filter)filter.onsubmit=e=>{e.preventDefault();const data=new FormData(filter);blockQuery=String(data.get('symbol')??'').trim();blockSide=String(data.get('side')??'all');paint();};
 }else{
  const records=Object.values(data.extras??{}).filter(r=>r.command==='get_contract_inventory'&&r.args?.symbol===selected&&r.observedMs<=now).sort((a,b)=>b.observedMs-a.observedMs);
  const archived=(frame as LiveState&{contractChain?:{observation:Extra}}).contractChain?.observation;
  const report=(archived?.value??records[0]?.value) as EtfReport|undefined;
  const weeklyReport=(frame as LiveState&{weeklyRangeSnapshot?:EtfReport}).weeklyRangeSnapshot??report;
  if(view==='weekly'){content.innerHTML=savedWeeklyRanges(selected,weeklyReport,now);return;}
  const dates=report?.expirations??[];
  const scoped=report&&report.symbol===selected?(expiry?scopeContractReport(report,[expiry]):report):undefined;
  content.innerHTML=`<section class="panel"><h2>Radar de inversión · Cadena de opciones · ${esc(selected)}</h2><p>Cadena guardada por el escritorio. El filtro consulta únicamente este lote.</p>${report?`<p>Fecha de consulta: ${new Date(report.at).toLocaleString('es',{timeZone:'America/New_York'})} ET ${now-report.at>300000?'· Datos anteriores: pendiente de una consulta más reciente.':''}</p>`:''}<label>Vencimiento <select id="saved-expiry"><option value="">Todos los guardados</option>${dates.map(d=>`<option value="${esc(d)}" ${d===expiry?'selected':''}>${esc(d)}</option>`).join('')}</select></label><div id="saved-chain">${optionOiHeatmapPanel(selected,scoped,report?.at??now)}</div>${!report?'<p>Cadena completa pendiente para este activo en el lote. No se consulta Data ni se inventan contratos.</p>':''}</section>`;
  content.insertAdjacentHTML('afterbegin',savedWeeklyRanges(selected,weeklyReport,now));
  bindOptionOiHeatmap(document.querySelector<HTMLElement>('#saved-chain')!,paint);
  document.querySelector<HTMLSelectElement>('#saved-expiry')!.onchange=e=>{expiry=(e.target as HTMLSelectElement).value;paint()};
 }
 document.querySelectorAll<HTMLButtonElement>('[data-symbol]').forEach(b=>b.setAttribute('aria-pressed',String(b.dataset.symbol===selected)));
}
async function pull(){
 const token=++generation,requested=view==='blocks'?'QQQ':selected;
 try{
  const response=await fetch(`/api/frame?symbol=${encodeURIComponent(requested)}`,{cache:'no-store',signal:AbortSignal.timeout(60000)});
  if(!response.ok){
   const failure=await response.json().catch(()=>({}));if(token!==generation)return;
   if(failure.error==='database_quota_exceeded'){
    document.querySelector<HTMLElement>('#web-status')!.textContent='Neon: cuota excedida · comprobación automática cada 5 min de lunes a viernes ET';
    if(!saved)content.innerHTML='<section class="panel">Neon ha bloqueado la base de datos porque se superó una cuota. Hay que revisar los límites o el plan del proyecto en Neon. La web volverá a comprobar la conexión automáticamente.</section>';
    return;
   }
   throw Error('Base de datos no disponible');
  }
  const value=await response.json() as Saved;if(token!==generation)return;
  if(!value.available){document.querySelector<HTMLElement>('#web-status')!.textContent=`${selected} · pendiente de datos guardados`;if(!saved)content.innerHTML=`<section class="panel">${esc(selected)} todavía no tiene un snapshot disponible en Neon. Se comprobará automáticamente; no se consultará Data desde la web.</section>`;setTimeout(()=>{if(token===generation&&webAutoRefreshAllowed())void pull()},15000);return;}
  if(value.frame.symbol!==requested||value.capturedMs>Date.now())throw Error('Lote inválido');
  saved=value;paint();
 }catch(e){if(token!==generation)return;document.querySelector<HTMLElement>('#web-status')!.textContent='Conexión pendiente · reintento automático';if(!saved)content.innerHTML=`<section class="panel">${esc(String(e))}</section>`;setTimeout(()=>{if(token===generation&&webAutoRefreshAllowed())void pull()},15000);}
}
document.querySelectorAll<HTMLButtonElement>('[data-symbol]').forEach(b=>b.onclick=()=>{selected=b.dataset.symbol!;saved=undefined;expiry='',blockQuery='',blockSide='all';void pull()});
document.querySelectorAll<HTMLButtonElement>('[data-view]').forEach(b=>b.onclick=()=>{view=b.dataset.view as 'detailed'|'chain'|'blocks'|'weekly';if(saved&&saved.frame.symbol===(view==='blocks'?'QQQ':selected))paint();else void pull()});
setInterval(()=>{if(!document.hidden&&webAutoRefreshAllowed())void pull()},300000);void pull();
// Expire visible levels at their observation deadline even if the next
// database request fails or the browser has not reached its five-minute poll.
setInterval(()=>{
 if(view!=='detailed'||!saved?.available||!content.querySelector('#live-workspace'))return;
 const snapshot=saved.frame.optionSnapshotStatus;
 const clock=Date.now();
 if(saved.capturedMs>clock+1000||clock-saved.capturedMs>15*60_000||!currentMapHorizon(snapshot,clock))paint();
},30000);
document.addEventListener('visibilitychange',()=>{if(!document.hidden&&webAutoRefreshAllowed())void pull()});





