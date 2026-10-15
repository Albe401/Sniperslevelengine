import {Channel,invoke,isTauri} from './snapshot-observation';
import {escapeHtml as esc,price} from './model';
export interface HistoricalRead {symbol:string;date:string;status:string;minuteCount:number;expectedMinutes:number;coverage:number;openFirstMinute:number|null;closeLastMinute:number|null;sessionChangePct:number|null;sessionHigh:number|null;sessionLow:number|null;shareVolume:number|null;notionalApprox:number|null;rvol:number|null;rvolStatus:string}
export interface HistoricalReport {date:string;phase:string;total:number;processed:number;rows:{symbol:string;summary:HistoricalRead|null;error:string|null}[];top:HistoricalRead[];error:string|null;savedPath:string|null}
const yesterday=()=>new Date(Date.parse(new Date().toLocaleDateString('en-CA',{timeZone:'America/New_York'})+'T12:00:00Z')-86_400_000).toISOString().slice(0,10);
let date=yesterday(),includeRvol=true,busy=false,message='',report:HistoricalReport|undefined,generation=0;
let channel:Channel<HistoricalReport>|undefined;
const value=(n:number|null)=>n==null?'—':price(n);
const quantity=(n:number|null)=>n==null?'—':Intl.NumberFormat('es',{notation:'compact',maximumFractionDigits:2}).format(n);
export function historicalContent(data:HistoricalReport|undefined,loading:boolean,error:string,selectedDate:string,available:boolean,rvol:boolean):string {
  const phase:Record<string,string>={loading:'Consultando actividad',rvol:'Calculando RVOL del Top 5',complete:'Consulta completada',partial:'Consulta parcial',paused:'Consulta pausada por la fuente',error:'Consulta no disponible',cancelled:'Consulta cancelada'};
  return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">SESIONES ANTERIORES · MASSIVE</span><h2>Búsqueda histórica</h2></div><span class="tag">CONTEXTO · SIN ENTRADAS</span></div>
  <div class="history-controls"><label>Fecha de Nueva York <input data-history-date type="date" min="2026-01-01" max="${yesterday()}" value="${esc(selectedDate)}" ${loading?'disabled':''}></label> <label><input type="checkbox" data-history-rvol ${rvol?'checked':''} ${loading?'disabled':''}> RVOL para los cinco finales</label> <button class="replay-button" data-history-action="start" ${loading||!available?'disabled':''}>Consultar sesión</button> <button class="replay-button" data-history-action="stop" ${!loading?'disabled':''}>Cancelar</button></div>
  <div class="panel-caption">${data?`${esc(phase[data.phase]??data.phase)} · ${data.processed} / ${data.total} activos · ${esc(data.date)}`:'Selecciona una sesión pasada. Calendario verificado desde 2026.'}${!available?'<br>Abre la aplicación de Windows para consultar Massive.':''}${error?`<br>${esc(error)}`:''}${data?.error?`<br>${esc(data.error)}`:''}</div>
  <div class="panel-head"><h2>Top 5 · actividad histórica</h2><span class="tag">${!data?'SIN CONSULTA':data.phase==='complete'?'UNIVERSO CONSULTADO':'RESULTADO PARCIAL'}</span></div>
  <div class="table-wrap"><table><thead><tr><th>ACTIVO</th><th>VOLUMEN · ACCIONES</th><th>NOCIONAL APROX.</th><th>VARIACIÓN</th><th>RANGO</th><th>RVOL</th><th>COBERTURA</th></tr></thead><tbody>${data?.top.map(r=>`<tr><td>${esc(r.symbol)}</td><td>${quantity(r.shareVolume)}</td><td>${quantity(r.notionalApprox)}</td><td>${value(r.sessionChangePct)} %</td><td>${value(r.sessionLow)} – ${value(r.sessionHigh)}</td><td>${value(r.rvol)}<small class="exposure-dates">${esc(r.rvolStatus)}</small></td><td>${r.minuteCount} / ${r.expectedMinutes}</td></tr>`).join('')||'<tr><td colspan="7">Sin resultados históricos disponibles.</td></tr>'}</tbody></table></div>
  <div class="panel-caption">Ranking descriptivo por nocional de la sesión regular; el nocional usa VWAP de cada minuto o su cierre cuando falta. No mide spread ni profundidad de opciones. RVOL compara el volumen acumulado con cinco sesiones previas comparables. Las sesiones fragmentadas quedan fuera del ranking; cierres anticipados respetan el calendario.</div>
  ${data?`<details><summary class="panel-caption">Ver cobertura de todos los activos</summary><div class="table-wrap"><table><thead><tr><th>ACTIVO</th><th>ESTADO</th><th>MINUTOS</th></tr></thead><tbody>${data.rows.map(r=>`<tr><td>${esc(r.symbol)}</td><td>${esc(r.error??r.summary?.status??'Sin datos')}</td><td>${r.summary?`${r.summary.minuteCount} / ${r.summary.expectedMinutes}`:'—'}</td></tr>`).join('')}</tbody></table></div></details>`:''}
  ${data?.savedPath?`<p class="panel-caption">Reporte guardado: ${esc(data.savedPath)}</p>`:''}</section>`;
}
export const historicalPage=()=>`<div id="historical-search">${historicalContent(report,busy,message,date,isTauri(),includeRvol)}</div>`;
function paint(){const root=document.querySelector('#historical-search');if(root){root.innerHTML=historicalContent(report,busy,message,date,isTauri(),includeRvol);bindHistorical();}}
export function bindHistorical(){
  const input=document.querySelector<HTMLInputElement>('[data-history-date]');if(input)input.onchange=()=>{date=input.value;};
  const check=document.querySelector<HTMLInputElement>('[data-history-rvol]');if(check)check.onchange=()=>{includeRvol=check.checked;};
  document.querySelectorAll<HTMLButtonElement>('[data-history-action]').forEach(button=>button.onclick=async()=>{
    if(button.dataset.historyAction==='stop'){generation++;try{await invoke('stop_historical_search');message='Consulta cancelada.';if(report)report={...report,phase:'cancelled'};}catch(e){message=String(e);}busy=false;channel=undefined;paint();return;}
    const token=++generation;busy=true;message='';report=undefined;paint();
    channel=new Channel<HistoricalReport>();channel.onmessage=data=>{if(token!==generation||data.date!==date)return;report=data;if(['complete','partial','paused','error'].includes(data.phase))busy=false;paint();};
    try{await invoke('start_historical_search',{date,includeRvol,onMessage:channel});}catch(e){if(token===generation){message=String(e);busy=false;paint();}}
  });
}
