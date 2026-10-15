import {quickChartLevels} from './quick-chart-levels';
import {quickReadingPanels} from './quick-reading-panels';
import {bindLevelCardSpacing} from './level-card-spacing';
import {buildIntradayControl} from './intraday-control';
import {bindGammaLive} from './gamma-live';
import {optionOiHeatmapPanel,bindOptionOiHeatmap} from './option-oi-heatmap-panel';
import {scopeContractReport} from './contract-profile';
import {seedCapturedAnalystTargets} from './analyst-targets';
import {seedCapturedIvHistory} from './iv-history';
import {seedCapturedComponentRead,type ComponentRead} from './qqq-components';
import type {TargetRow} from './analyst-targets-cache';
import type {EtfReport} from './etf-dealers';
import type {LiveState} from './live';
import {escapeHtml as esc} from './model';
import './day-map.css';
import './styles.css';import './restrained.css';import './visual-readings.css';import './brand.css';import './route-design.css';import './reading-tabs.css';import './compact-reading-height.css';import '@fontsource-variable/inter';import './web.css';
const symbols=['AAPL','MSFT','AMZN','GOOGL','META','NVDA','TSLA','QQQ','SPY','IWM'];
type Extra={command:string;args?:{symbol?:string};observedMs:number;value:unknown};
type Saved={available:boolean;id:string;capturedMs:number;frame:LiveState;extras:Record<string,Extra>;coverage:Record<string,unknown>};
let selected='QQQ',view:'quick'|'detailed'|'chain'='quick',generation=0,saved:Saved|undefined,expiry='';
const app=document.querySelector<HTMLDivElement>('#app')!;
app.innerHTML=`<div class="web-shell"><aside class="web-sidebar"><img src="/assets/route-lion.png" alt="Snipers"><h2>SNIPERS</h2><button data-view="quick">Lectura rápida</button><button data-view="detailed">Lectura detallada</button><button data-view="chain">Radar de inversión<br><small>Cadena de opciones</small></button><small>Datos guardados por el escritorio<br>Actualización cada cinco minutos</small></aside><main class="web-main"><header class="web-topbar"><strong>Algoritmo de probabilidad</strong><span id="web-status">Leyendo la base de datos…</span></header><nav class="web-assets">${symbols.map(s=>`<button data-symbol="${s}">${s}</button>`).join('')}</nav><section id="web-content"></section></main></div>`;
const content=document.querySelector<HTMLElement>('#web-content')!;
function paint(){
 if(!saved?.available)return;
 const data=saved,frame=data.frame,now=data.capturedMs;
 const fields=frame as LiveState&{ivHistory?:{observation:Extra};qqqReport?:ComponentRead};
 const iv=fields.ivHistory?.observation;
 seedCapturedIvHistory(iv&&iv.observedMs<=now?iv.value:undefined,selected,now);
 seedCapturedComponentRead(frame.qqq_report??fields.qqqReport,now);
 const targets=Object.values(data.extras??{}).filter(r=>r.command==='get_fmp_targets'&&r.observedMs<=now).sort((a,b)=>b.observedMs-a.observedMs)[0];
 if(targets&&(targets.value as {rows?:unknown})?.rows)seedCapturedAnalystTargets((targets.value as {rows:Record<string,TargetRow>}).rows,now);
 document.querySelector<HTMLElement>('#web-status')!.textContent=`Captura ${new Date(now).toLocaleString('es',{timeZone:'America/New_York'})} ET · actualización automática cada 5 min`;
 if(view==='quick'){
  content.innerHTML=`<section id="live-workspace" class="visual-reading" data-live-view="quick">${quickChartLevels(frame,selected,true,now,buildIntradayControl(frame,selected,true,now))}</section><p class="web-note">Solo datos del lote guardado. Sin conexión a Massive desde la web. Fechas y cobertura originales conservadas.</p>`;
  bindGammaLive(frame,selected,true,paint,now);
 }else if(view==='detailed'){
  content.innerHTML=`<section id="live-workspace" class="visual-reading" data-live-view="detailed">${quickReadingPanels(frame,selected,true,buildIntradayControl(frame,selected,true,now),now)}</section><p class="web-note">Lectura detallada del lote guardado en Neon. Actualización cada cinco minutos; sin consultas al mercado desde la web.</p>`;
  bindLevelCardSpacing();bindGammaLive(frame,selected,true,paint,now);
 }else{
  const records=Object.values(data.extras??{}).filter(r=>r.command==='get_contract_inventory'&&r.args?.symbol===selected&&r.observedMs<=now).sort((a,b)=>b.observedMs-a.observedMs);
  const archived=(frame as LiveState&{contractChain?:{observation:Extra}}).contractChain?.observation;
  const report=(archived?.value??records[0]?.value) as EtfReport|undefined;
  const dates=report?.expirations??[];
  const scoped=report&&report.symbol===selected?(expiry?scopeContractReport(report,[expiry]):report):undefined;
  content.innerHTML=`<section class="panel"><h2>Radar de inversión · Cadena de opciones · ${esc(selected)}</h2><p>Cadena guardada por el escritorio. El filtro consulta únicamente este lote.</p>${report?`<p>Fecha de consulta: ${new Date(report.at).toLocaleString('es',{timeZone:'America/New_York'})} ET ${now-report.at>300000?'· Datos anteriores: pendiente de una consulta más reciente.':''}</p>`:''}<label>Vencimiento <select id="saved-expiry"><option value="">Todos los guardados</option>${dates.map(d=>`<option value="${esc(d)}" ${d===expiry?'selected':''}>${esc(d)}</option>`).join('')}</select></label><div id="saved-chain">${optionOiHeatmapPanel(selected,scoped,report?.at??now)}</div>${!report?'<p>Cadena completa pendiente para este activo en el lote. No se consulta Massive ni se inventan contratos.</p>':''}</section>`;
  bindOptionOiHeatmap(document.querySelector<HTMLElement>('#saved-chain')!,paint);
  document.querySelector<HTMLSelectElement>('#saved-expiry')!.onchange=e=>{expiry=(e.target as HTMLSelectElement).value;paint()};
 }
 document.querySelectorAll<HTMLButtonElement>('[data-symbol]').forEach(b=>b.setAttribute('aria-pressed',String(b.dataset.symbol===selected)));
}
async function pull(){
 const token=++generation;
 try{
  const response=await fetch(`/api/frame?symbol=${encodeURIComponent(selected)}`,{cache:'no-store',signal:AbortSignal.timeout(60000)});
  if(!response.ok)throw Error('Base de datos no disponible');
  const value=await response.json() as Saved;if(token!==generation)return;
  if(!value.available){document.querySelector<HTMLElement>('#web-status')!.textContent='Esperando datos guardados · reintento automático';if(!saved)content.innerHTML='<section class="panel">Todavía no hay datos guardados. Reintentando automáticamente.</section>';setTimeout(()=>{if(token===generation)void pull()},15000);return;}
  if(value.frame.symbol!==selected||value.capturedMs>Date.now())throw Error('Lote inválido');
  saved=value;paint();
 }catch(e){if(token!==generation)return;document.querySelector<HTMLElement>('#web-status')!.textContent='Conexión pendiente · reintento automático';if(!saved)content.innerHTML=`<section class="panel">${esc(String(e))}</section>`;setTimeout(()=>{if(token===generation)void pull()},15000);}
}
document.querySelectorAll<HTMLButtonElement>('[data-symbol]').forEach(b=>b.onclick=()=>{selected=b.dataset.symbol!;saved=undefined;expiry='';void pull()});
document.querySelectorAll<HTMLButtonElement>('[data-view]').forEach(b=>b.onclick=()=>{view=b.dataset.view as 'quick'|'detailed'|'chain';if(saved)paint();else void pull()});
setInterval(()=>{if(!document.hidden)void pull()},300000);void pull();
document.addEventListener('visibilitychange',()=>{if(!document.hidden)void pull()});
