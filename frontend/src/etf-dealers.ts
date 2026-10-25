import {componentCard} from './qqq-components';
import {invoke,isTauri} from './snapshot-observation';
import {scrollStrikeCenter,strikeProfileWindow} from './strike-profile-scroll';
import type {CvdRead} from './cvd';
import type {LiveState} from './live';
import {escapeHtml as esc} from './model';
import './etf-dealers.css';
import {expiryReport,rankStrikes,enrichmentPanel,etfFlowPanel,etfBlocksPanel,type Enrichment,type EtfFlow} from './etf-enrichment';

export interface EtfSide {oi:number|null;volume:number|null;delta:number|null;gamma:number|null}
export interface EtfStrike {strike:number;call:EtfSide;put:EtfSide}
export interface MovementRoom {symbol:string;at:number;available:boolean;anchored:boolean;expectedMoveDollars:number|null;expectedMovePct:number|null;reference:number|null;lower:number|null;upper:number|null;callDollars:number|null;putDollars:number|null;callPct:number|null;putPct:number|null;extensionCall:number|null;extensionPut:number|null;reason:string}
export interface EtfReport {gexWindows?:import('./gex-windows').GexWindowReport|null;heatmap?:{reference:unknown[];snapshots:unknown[];referenceComplete:boolean;scopeStart:string;scopeEnd:string}|null;dailyRange?:import('./daily-range').DailyRange|null;amplitude?:MovementRoom|null;enrichment?:Enrichment|null;symbol:string;days:number;at:number;spot:number;priceAt:number|null;expirations:string[];contracts:number;rows:EtfStrike[];gammaComplete:boolean;deltaComplete:boolean}
type Metric=keyof EtfSide;
const labels:Record<Metric,string>={oi:'OI',volume:'Volumen',delta:'Delta',gamma:'Gamma / GEX'};
const units:Record<Metric,string>={oi:'contratos abiertos',volume:'contratos negociados · snapshot',delta:'acciones equivalentes · exposición absoluta',gamma:'USD por movimiento del 1 %'};
export const etfSymbols=['SPY','QQQ'] as const;
let days=0,busy=false,generation=0,visible=false;
let lifecycle:Promise<unknown>=Promise.resolve();
export function setEtfScannerVisible(next:boolean){
 if(next===visible)return;visible=next;generation++;
 if(!next){cvds={};optionFlows={};blockReports={};connected=false;}
 if(isTauri())lifecycle=lifecycle.catch(()=>undefined).then(()=>invoke('set_etf_scanner_enabled',{enabled:next}));
 void lifecycle.catch(error=>{etfSymbols.forEach(s=>errors[s]=String(error));});
}
const expiryFilters:Record<string,string>={SPY:'',QQQ:''};
let optionFlows:Record<string,EtfFlow|null>={};
let blockReports:Record<string,import("./block-pressure").BlockReport>={},blockAt:number|undefined;
const metrics:Record<string,Metric>={SPY:'oi',QQQ:'oi'};
const reports:Record<string,EtfReport>={},errors:Record<string,string>={};
let cvds:Record<string,CvdRead>={},connected=false;
const centers:Record<string,number|undefined>={},picked:Record<string,number|undefined>={};
const key=(symbol:string)=>`${symbol}:${days}`;
const etDay=(at:number)=>new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York',year:'numeric',month:'2-digit',day:'2-digit'}).format(at);
const number=(n:number|null)=>n==null?'—':new Intl.NumberFormat('es',{notation:'compact',maximumFractionDigits:2}).format(n);
export function updateEtfCvd(reads:Record<string,CvdRead>|undefined,active:boolean,frame?:LiveState){if(!visible)return;cvds=reads??{};connected=active;optionFlows=frame?.etfFlow??{};blockReports=frame?.etfBlocks??{};blockAt=frame?.publishedMs;}
export function usableEtfReport(r:EtfReport|undefined,s:string,d:number,now=Date.now()):r is EtfReport {
 return !!r&&r.symbol===s&&r.days===d&&Number.isFinite(r.at)&&r.at<=now&&now-r.at<=300000&&etDay(r.at)===etDay(now)&&r.spot>0&&Number.isFinite(r.spot)&&r.rows.every(row=>Number.isFinite(row.strike)&&row.strike>0)&&new Set(r.rows.map(row=>row.strike)).size===r.rows.length;
}
export function etfProfileSvg(r:EtfReport|undefined,m:Metric,center:number|undefined,selected:number|undefined,height=600,aligned?:{strike:number;y:number}[]):string{
 const rows=r?.rows??[],positions=aligned??strikeProfileWindow(rows.map(row=>row.strike),center??r?.spot??0,height);
 const valid=(v:number|null)=>v!=null&&Number.isFinite(v)&&v>=0?v:null;
 const peak=Math.max(1,...rows.flatMap(row=>[valid(row.call[m])??0,valid(row.put[m])??0]));
 return `<svg viewBox="0 0 460 ${height}" aria-label="Perfil ${labels[m]} por strike" role="img"><text x="105" y="35" text-anchor="middle" fill="#ff8795">PUT</text><text x="330" y="35" text-anchor="middle" fill="#c7a2ff">CALL</text><line x1="220" x2="220" y1="65" y2="${height-25}" stroke="#73839b" stroke-dasharray="4 4"/>${positions.map(({strike,y})=>{
 const row=rows.find(row=>row.strike===strike)!;const put=valid(row.put[m]),call=valid(row.call[m]);
 return `<g data-etf-strike="${strike}" tabindex="0" role="button" aria-label="Strike ${strike}, PUT ${number(put)}, CALL ${number(call)}"><title>Strike ${strike} · ${labels[m]} · PUT ${number(put)} · CALL ${number(call)}</title><rect x="0" y="${y-10}" width="460" height="21" fill="${selected===strike?'#ba8aff26':'transparent'}"/>${put==null?'':`<rect x="${220-put/peak*116}" y="${y-5}" width="${put/peak*116}" height="10" fill="#ff6273"/>`}${call==null?'':`<rect x="220" y="${y-5}" width="${call/peak*116}" height="10" fill="#a45aff"/>`}<text x="94" y="${y+4}" text-anchor="end" fill="#ff8795" font-size="11">${number(put)}</text><text x="345" y="${y+4}" fill="#c7a2ff" font-size="11">${number(call)}</text><text x="455" y="${y+4}" text-anchor="end" fill="#d4e2f1" font-size="11">${strike.toFixed(2)}</text></g>`;
 }).join('')||`<text x="230" y="300" text-anchor="middle" fill="#a6c7e4">Sin strikes en este horizonte</text>`}</svg>`;
}
function profileContent(symbol:string):string{
 const report=reports[key(symbol)],r=expiryReport(usableEtfReport(report,symbol,days)?report:undefined,expiryFilters[symbol]);
 const metric=metrics[symbol],row=r?.rows.find(row=>row.strike===picked[key(symbol)]);
 return `<div class="etf-profile-heading"><div class="etf-profile-head"><h2>${symbol}</h2><small>${r?`Referencia $${r.spot.toFixed(2)} · ${r.rows.length} strikes · ${r.contracts} contratos`:'Esperando cadena propia'}</small><small>${units[metric]}</small></div>${symbol==='QQQ'?`<div data-qqq-components>${componentCard()}</div>`:''}</div><label class="etf-expiry">Vencimiento <select data-etf-expiry><option value="">Todos del horizonte</option>${(report?.expirations??[]).map(e=>`<option value="${esc(e)}" ${expiryFilters[symbol]===e?'selected':''}>${esc(e)}</option>`).join('')}</select></label><div class="etf-metrics">${Object.entries(labels).map(([value,label])=>`<button data-etf-metric="${value}" aria-pressed="${metric===value}">${label}</button>`).join('')}</div><div class="etf-ranking"><small>Mayores ${labels[metric]} · CALL + PUT</small>${rankStrikes(r,metric).map(item=>`<button data-etf-jump="${item.strike}">${item.strike.toFixed(2)} · ${number(item.value)}</button>`).join('')}</div><div class="etf-scroll-tools"><button data-etf-scroll="-1" aria-label="Strikes superiores de ${symbol}">↑</button><button data-etf-center>Centrar en precio</button><button data-etf-scroll="1" aria-label="Strikes inferiores de ${symbol}">↓</button></div><p class="etf-card-status" role="status">${esc(errors[symbol]||(busy?'Consultando…':r?`Snapshot ${new Date(r.at).toLocaleTimeString('es',{timeZone:'America/New_York'})} ET · ${r.expirations.length} vencimientos`:'Sin exposición vigente de este horizonte'))}</p><div data-etf-profile-wheel>${etfProfileSvg(r,metric,centers[key(symbol)],picked[key(symbol)])}</div><small>Rueda ↑↓ · todos los strikes consultados · clic para ver cantidades</small><div class="etf-strike-detail"><h3>${row?`Strike $${row.strike.toFixed(2)}`:'Selecciona un strike'}</h3><table><thead><tr><th>Métrica</th><th>PUT</th><th>CALL</th></tr></thead><tbody>${(Object.keys(labels) as Metric[]).map(m=>`<tr><th>${labels[m]}</th><td>${number(row?.put[m]??null)}</td><td>${number(row?.call[m]??null)}</td></tr>`).join('')}</tbody></table></div>${enrichmentPanel(usableEtfReport(report,symbol,days)?report:undefined,picked[key(symbol)],expiryFilters[symbol])}<div class="etf-flow">${etfFlowPanel(optionFlows[symbol],symbol,isTauri()&&connected)}${etfBlocksPanel(blockReports[symbol],symbol,blockAt,isTauri()&&connected)}</div>`;
}
export function etfDealersPage():string{
 return `<section class="etf-scanner"><header class="scanner-heading"><div><span class="eyebrow">SNIPERS · CADENA DE OPCIONES</span><h1>Scanner SPY / QQQ</h1><p>Acumulación CALL / PUT por strike.</p></div><button class="primary-button" data-etf-refresh ${busy||!isTauri()?'disabled':''}>${busy?'Consultando…':'Actualizar cadenas'}</button></header><div class="etf-toolbar"><label>Horizonte <select data-etf-horizon>${[[0,'0DTE · hoy'],[7,'Semanal · 0–7 días'],[28,'4 semanas · 0–28 días'],[30,'Un mes · 0–30 días'],[183,'Todas · hasta 6 meses']].map(([value,label])=>`<option value="${value}" ${days===value?'selected':''}>${label}</option>`).join('')}</select></label><small>Rueda sobre cada perfil para recorrer sus strikes</small></div><p data-etf-status role="status">${!isTauri()?'Abre Snipers Windows para consultar datos.':'Cadena directa Data · motores Snipers'}</p><div class="etf-profiles">${etfSymbols.map(symbol=>`<article class="etf-profile" data-etf-profile="${symbol}" aria-label="Contratos de ${symbol}">${profileContent(symbol)}</article>`).join('')}</div><details><summary>Método y datos</summary><p>OI = contratos abiertos; volumen = acumulado del snapshot; delta = OI × |delta| × multiplicador en acciones equivalentes; gamma/GEX = gamma × OI × multiplicador × precio² × 1%, en USD. PUT a la izquierda, CALL a la derecha. Datos ausentes: —. No identifica inventario real de market makers.</p><p>Cadena directa de Data y exposición nativa Snipers. Todos los strikes recibidos; horizontes acumulados: hoy, 0–7 días, 0–28 días, 0–30 días o hasta seis meses naturales. 0DTE vacío no sustituye otro vencimiento. Actualización cada minuto; consulta caducada a los cinco minutos.</p><p>Bloques simultáneos SPY/QQQ: muestra en vivo desde la conexión, sin historial anterior reconstruido. Flujo de opciones: muestra compartida Snipers para ambos ETFs, con cobertura por ventana. Charm/vanna son sensibilidades de modelo. OI y griegas conservan su semántica de snapshot; no certifican nuevas posiciones intradía.</p></details></section>`;
}
function bindProfile(host:HTMLElement,symbol:string){
 const expiry=host.querySelector<HTMLSelectElement>('[data-etf-expiry]');if(expiry)expiry.onchange=()=>{expiryFilters[symbol]=expiry.value;centers[key(symbol)]=undefined;paintProfile(symbol);};
 host.querySelectorAll<HTMLButtonElement>('[data-etf-jump]').forEach(b=>b.onclick=()=>{centers[key(symbol)]=Number(b.dataset.etfJump);picked[key(symbol)]=Number(b.dataset.etfJump);paintProfile(symbol);});
 host.querySelectorAll<HTMLButtonElement>('[data-etf-metric]').forEach(b=>b.onclick=()=>{metrics[symbol]=b.dataset.etfMetric as Metric;paintProfile(symbol);});
 const move=(delta:number)=>{const r=expiryReport(reports[key(symbol)],expiryFilters[symbol]);if(!usableEtfReport(r,symbol,days))return;centers[key(symbol)]=scrollStrikeCenter(r.rows.map(row=>row.strike),centers[key(symbol)],r.spot,delta);paintProfile(symbol);};
 const wheel=host.querySelector<HTMLElement>('[data-etf-profile-wheel]');if(wheel)wheel.onwheel=e=>{e.preventDefault();e.stopPropagation();move(e.deltaY*(e.deltaMode===1?22:e.deltaMode===2?600:1));};
 host.querySelectorAll<HTMLButtonElement>('[data-etf-scroll]').forEach(b=>b.onclick=()=>move(Number(b.dataset.etfScroll)*40));
 const center=host.querySelector<HTMLButtonElement>('[data-etf-center]');if(center)center.onclick=()=>{centers[key(symbol)]=undefined;paintProfile(symbol);};
 host.querySelectorAll<SVGElement>('[data-etf-strike]').forEach(g=>{const pick=()=>{picked[key(symbol)]=Number(g.dataset.etfStrike);paintProfile(symbol);};g.onclick=pick;g.onkeydown=e=>{if(e.key==='Enter'||e.key===' '){e.preventDefault();pick();}};});
}
function paintProfile(symbol:string){
 const host=document.querySelector<HTMLElement>(`[data-etf-profile="${symbol}"]`);if(!host)return;
 const open=[...host.querySelectorAll('details')].map(d=>d.open);
 host.innerHTML=profileContent(symbol);host.querySelectorAll('details').forEach((d,i)=>d.open=open[i]??false);bindProfile(host,symbol);
}
function paint(){
 if(!document.querySelector('[data-etf-profile]'))return;
 etfSymbols.forEach(paintProfile);
 const b=document.querySelector<HTMLButtonElement>('[data-etf-refresh]');if(b){b.disabled=busy||!isTauri();b.textContent=busy?'Consultando…':'Actualizar cadenas';}
}
async function refresh(){
 if(!visible||busy||!isTauri()||!document.querySelector('[data-etf-profile]'))return;
 const token=generation,d=days;busy=true;etfSymbols.forEach(s=>errors[s]='');paint();
 const result=await Promise.allSettled(etfSymbols.map(async symbol=>{await lifecycle;if(!visible||token!==generation)throw new Error('Scanner inactivo');return invoke<EtfReport>('get_etf_scanner',{symbol,days:d});}));
 if(token===generation)result.forEach((r,i)=>{const s=etfSymbols[i];if(r.status==='fulfilled')reports[`${s}:${d}`]=r.value;else errors[s]=String(r.reason);});
 busy=false;paint();if(token!==generation)void refresh();
}
export function bindEtfDealers(){
 const b=document.querySelector<HTMLButtonElement>('[data-etf-refresh]');if(!b)return;b.onclick=()=>{void refresh();};
 const h=document.querySelector<HTMLSelectElement>('[data-etf-horizon]');if(h)h.onchange=()=>{days=Number(h.value);generation++;etfSymbols.forEach(s=>expiryFilters[s]='');etfSymbols.forEach(s=>errors[s]='');h.blur();paint();void refresh();};paint();void refresh();
}
setInterval(()=>{void refresh();},60000);
setInterval(()=>{if(document.querySelector('[data-etf-profile]'))paint();},1000);
