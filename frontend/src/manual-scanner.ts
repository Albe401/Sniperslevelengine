import {rankNewsAssets,assetNewsPanel,topFiveNewsMethod} from './top-five-news';
import {nativeHorizon,setNativeHorizon,shortlistNative,nativeOptionsPanel,refreshNativeOptions} from './native-top-five';
import {topFiveVolumeMetrics,type TopFiveVolumeMetrics} from './top-five-volume-metrics';
import {Channel,invoke,isTauri} from './snapshot-observation';
import {escapeHtml as esc,price} from './model';
import './manual-scanner.css';
export interface ManualCandidate {symbol:string;sector:string;volume:number;price:number;changePct:number;lastBarMs:number;bars:number;volumeMetrics?:TopFiveVolumeMetrics}
export interface ManualReport {date:string;startMs:number;cutoffMs:number;total:number;processed:number;succeeded:number;empty:number;errors:string[];candidates:ManualCandidate[];complete:boolean}
export const manualSearch:{busy:boolean;error:string;report?:ManualReport}={busy:false,error:''};
let requestId=0;

const time=new Intl.DateTimeFormat('es',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit',hourCycle:'h23'});
const day=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York',year:'numeric',month:'2-digit',day:'2-digit'});
const number=new Intl.NumberFormat('es',{maximumFractionDigits:0});
export function manualScannerContent(report:ManualReport|undefined,busy:boolean,error:string,available:boolean,now=Date.now()):string {
 const prior=report&&report.date!==day.format(now);
 const rows=prior?shortlistNative(report?.candidates??[]).slice(0,5):rankNewsAssets(report?.candidates??[],now);
 return `<div class="manual-scan-heading"><div><span class="eyebrow">BÚSQUEDA A TU HORA</span><h2>Top 5 · búsqueda manual</h2><p>Volumen de premarket desde las 04:00 ET hasta el último minuto cerrado, con corte máximo a las 09:30 ET. Todo el universo configurado. Opciones nativas sobre hasta 20 candidatos por volumen; Top 5 por opciones y catalizadores de noticias.</p><label>Opciones <select data-native-horizon>${[[0,'0DTE · hoy'],[7,'0–7 días'],[28,'0–28 días']].map(([v,label])=>`<option value="${v}" ${nativeHorizon===v?'selected':''}>${label}</option>`).join('')}</select></label></div><button class="manual-scan-button" data-manual-scan ${busy||!available?'disabled':''}>${busy?'Buscando…':'Buscar ahora'}</button></div>
 <p class="manual-scan-status" role="status">${!available?'Abre la aplicación de Windows para consultar Massive.':busy?`${report&&report.processed===report.total?'Consultando RVOL de hasta veinte candidatos':`Consultando Massive · ${report?.processed??0} / ${report?.total??'—'} activos`}`:report?`${prior?'Consulta de una sesión anterior · ':''}${report.complete?'Universo consultado':'Resultado parcial'} · ${esc(report.date)} · corte ${time.format(report.cutoffMs)} ET · ${report.succeeded} / ${report.total} consultas correctas · ${report.empty} sin volumen`:'Puedes buscar a las 07:00, 07:30, 08:00 o cuando quieras desde las 04:01 ET en un día de mercado.'}${error?` · ${esc(error)}`:''}</p>
 ${report?`<div class="manual-scan-results">${rows.map((r,i)=>`<article class="manual-scan-card"><div><span>0${i+1}</span><b>${esc(r.symbol)}</b><strong class="${r.changePct>=0?'is-up':'is-down'}">${r.changePct>=0?'+':''}${r.changePct.toFixed(2)}%</strong></div><small>${esc(r.sector||'Sin sector')}</small><h3>${number.format(r.volume)} <small>acciones</small></h3><p>Último cierre <b>$${price(r.price)}</b></p>${topFiveVolumeMetrics(r.volumeMetrics)}${nativeOptionsPanel(r,now,available&&!prior)}${assetNewsPanel(r,report.candidates,now,!prior)}<small>${time.format(r.lastBarMs)} ET · ${r.bars} minutos con datos</small></article>`).join('')||'<p>Sin activos con volumen recibido para este corte.</p>'}</div><p class="scanner-note">Foto de la consulta, no cotización en vivo. Preselección por volumen de acciones; orden por prioridad nativa cuando hay opciones completas; pendientes al final. ${topFiveNewsMethod} Índice descriptivo, no probabilidad de movimiento; la cantidad de minutos no certifica cobertura completa. ${busy?'Ranking provisional mientras se consulta el universo. ':''}No cambia la selección automática ni activa entradas.</p>${report.errors.length?`<details><summary>${report.errors.length} incidencias de consulta</summary><ul>${report.errors.map(e=>`<li>${esc(e)}</li>`).join('')}</ul></details>`:''}`:''}`;
}
export const manualScannerPanel=()=>`<section class="manual-scan" data-manual-scan-root aria-label="Búsqueda manual Top 5">${manualScannerContent(manualSearch.report,manualSearch.busy,manualSearch.error,isTauri(),Date.now())}</section>`;
function paint(){const root=document.querySelector('[data-manual-scan-root]');if(root){root.innerHTML=manualScannerContent(manualSearch.report,manualSearch.busy,manualSearch.error,isTauri(),Date.now());bindManualScanner();}}
export function bindManualScanner(){
 const horizon=document.querySelector<HTMLSelectElement>('[data-native-horizon]');if(horizon)horizon.onchange=()=>{setNativeHorizon(Number(horizon.value));paint();void refreshProfiles(true);};
 const button=document.querySelector<HTMLButtonElement>('[data-manual-scan]');
 if(button)button.onclick=async()=>{
  if(manualSearch.busy||!isTauri())return;
  const current=++requestId;
  manualSearch.busy=true;manualSearch.error='';manualSearch.report=undefined;paint();
  const channel=new Channel<ManualReport>();
  channel.onmessage=report=>{if(current!==requestId||!manualSearch.busy)return;manualSearch.report=report;paint();};
  try{manualSearch.report=await invoke<ManualReport>('search_top_five_now',{onMessage:channel});}
  catch(e){manualSearch.error=String(e);}
  finally{manualSearch.busy=false;paint();void refreshProfiles();}
 };
}

async function refreshProfiles(force=false){if(manualSearch.busy||!manualSearch.report||manualSearch.report.date!==day.format(Date.now()))return;await refreshNativeOptions(manualSearch.report.candidates,paint,force);}
if(typeof document!=='undefined')document.addEventListener('snipers:market-news',()=>paint());
setInterval(()=>{void refreshProfiles();},60000);
