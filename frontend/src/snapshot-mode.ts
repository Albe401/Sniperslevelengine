import {invoke,isTauri} from '@tauri-apps/api/core';
import {escapeHtml as esc} from './model';
import {snapshotSymbols,validSavedBundle,savedAsset,type SavedBundle} from './snapshot-model';
import './snapshot-mode.css';
let enabled=false,selected='QQQ',saved:SavedBundle|undefined,busy=false,error='',changed=()=>{};
export function snapshotEnabled(){return enabled;}
export function snapshotButton(){return `<button class="snapshot-toggle" data-snapshot-toggle aria-pressed="${enabled}">${enabled?'SNAPSHOT · ≥5 MIN':'En vivo'}</button>`;}
const date=(ms:number)=>new Date(ms).toLocaleString('es',{timeZone:'America/New_York',hour12:false});
const labels:Record<string,string>={ticks:'Precios y cotizaciones',metrics:'VWAP, volumen y actividad',chartHistory:'Historial del gráfico',minutes:'Velas por minuto',auctionMap:'AMT · aceptación, rechazo y perfil de volumen',optionSnapshotStatus:'Opciones · estructura y griegas',flow:'Flujo · Call Buy / Put Buy',cvd:'CVD · compra y venta',offExchange:'Bloques y operaciones fuera de bolsa',scanner:'Scanner y selección',dailyRange:'Rango diario',dailyAtr:'ATR diario',directionOi:'Posicionamiento de opciones',structuralPaths:'Ruta estructural',positionMaps:'Mapas de posición',wallFlow:'Flujo por nivel',wallPull:'Atracción por nivel',rangeContext:'Contexto de rangos',gapReport:'Gaps',spxReading:'Lectura SPX',qqq_report:'Componentes de QQQ',treasury:'Bonos del Tesoro',neuroContext:'Contexto del motor',levelDynamics:'Dinámica de niveles',stocks:'Estado de Stocks',options:'Estado de Options'};
const fieldLabels:Record<string,string>={price:'Precio',bid:'Bid',ask:'Ask',tradeMs:'Última operación · ET',quoteMs:'Última cotización · ET',asofMs:'Fecha de los datos · ET',observedMs:'Fecha de observación · ET',timestampMs:'Hora · ET',timestamp_ms:'Hora · ET',phase:'Estado',session:'Sesión',volume:'Volumen',baselineSessions:'Sesiones de referencia',vwap:'VWAP',rvol:'Volumen relativo',symbol:'Activo',get_benzinga_news:'Noticias',get_company_research_detail:'Estudio de la compañía',get_weekly_range_snapshot:'Rango semanal',get_fmp_targets:'Objetivos de analistas',get_company_research_catalog:'Catálogo de compañías'};
function content(value:unknown,depth=0,field=''):string {
 if(value==null)return '<span class="snapshot-missing">Sin datos guardados</span>';
 if(typeof value==='number'&&/Ms$|timestamp_ms$/.test(field)&&value>1e12)return esc(date(value));
 if(typeof value!=='object')return esc(typeof value==='number'?String(value):String(value));
 const rows=Array.isArray(value)?value.map((v,i)=>[String(i+1),v] as const):Object.entries(value);
 if(!rows.length)return '<span class="snapshot-missing">Sin registros en la captura</span>';
 if(depth>=2)return `<details><summary>${rows.length} campos / registros</summary><pre>${esc(JSON.stringify(value,null,2))}</pre></details>`;
 return `<div class="snapshot-values">${rows.map(([key,v])=>`<div><span>${esc(labels[key]??fieldLabels[key]??key)}</span><section>${content(v,depth+1,key)}</section></div>`).join('')}</div>`;
}
function profile(frame:Record<string,unknown>):string {
 const r=frame.auctionMap as {bins?:{low:number;high:number;volume:number;kind:string}[];pocLow?:number;pocHigh?:number;val?:number;vah?:number}|undefined;
 if(!r?.bins?.length)return '';
 const max=Math.max(...r.bins.map(b=>b.volume));
 return `<section class="panel snapshot-profile"><h3>Perfil AMT de la captura</h3><p>POC ${r.pocLow}–${r.pocHigh} · VAL ${r.val} · VAH ${r.vah}</p>${[...r.bins].reverse().map(b=>`<div><span>${b.low.toFixed(2)}</span><i style="width:${max>0?100*b.volume/max:0}%;background:${b.low===r.pocLow?'#a78bfa':b.kind==='acceptance'?'#10d994':'#32d6f5'}"></i></div>`).join('')}</section>`;
}
export function snapshotPage(view:string):string {
 return renderSavedSnapshot(saved,selected,view,Date.now(),error);
}
export function renderSavedSnapshot(saved:SavedBundle|undefined,selected:string,view:string,now:number,error=''):string {
 const frame=savedAsset(saved,selected,now);
 const extras=Object.values(saved?.extras??{}).filter(r=>r.observedMs<=(saved?.capturedMs??0)&&(!r.args.symbol||r.args.symbol===selected));
 const age=saved?.capturedMs?Math.floor((now-saved.capturedMs)/60000):null;
 const metadata=new Set(['symbol','startedMs','publishedMs','dataService','priceAlerts','journalRecords','overwrittenRecords','etfScannerEnabled','selection','snapshotUnavailable']);
 const priority=['ticks','metrics','auctionMap','optionSnapshotStatus','flow','cvd','offExchange','scanner','chartHistory','rawOptions'];
 const entries=Object.entries(frame??{}).filter(([key])=>!metadata.has(key)).sort(([a],[b])=>{
  const ai=priority.indexOf(a),bi=priority.indexOf(b);return (ai<0?100:ai)-(bi<0?100:bi);
 });
 const photograph=saved?.images?.[selected];
 return `<section class="snapshot-page"><header class="panel"><span class="eyebrow">CAPTURA GUARDADA · ${esc(selected)}</span><h2>Modo snapshot</h2><p>${saved?.capturedMs?`Última captura visible: ${date(saved.capturedMs)} ET · hace ${age} min.`:'Esperando una captura guardada con al menos cinco minutos de edad.'}</p><p>Esta vista muestra únicamente datos de la base de datos. El receptor interno continúa trabajando.</p>${error?`<p role="alert">${esc(error)}</p>`:''}<div class="snapshot-assets">${snapshotSymbols.map(s=>`<button data-snapshot-symbol="${s}" aria-pressed="${s===selected}">${s}</button>`).join('')}</div></header>
 ${!frame?'<section class="panel">No hay una captura válida disponible. No se sustituye por datos en vivo.</section>':`
 ${photograph?.mimeType==='image/jpeg'?`<section class="panel snapshot-photograph"><img alt="Lectura detallada de ${esc(selected)} en la captura guardada" src="data:image/jpeg;base64,${esc(photograph.base64.replace(/\s/g,''))}" /></section>`:'<section class="panel">Imagen de Lectura detallada pendiente para esta captura.</section>'}
 <details class="snapshot-data-details"><summary>Datos y cobertura de la misma captura</summary>
 ${frame.snapshotUnavailable?'<p class="snapshot-missing">Este activo estaba pendiente en el receptor al capturar.</p>':''}
 ${profile(frame)}<div class="snapshot-sections">${entries.map(([key,value])=>`<details class="panel" ${['ticks','metrics'].includes(key)?'open':''}><summary>${esc(labels[key]??key)}</summary>${content(value)}</details>`).join('')}</div>
 <section class="panel"><h3>Investigación, noticias y otras secciones guardadas</h3>${extras.length?extras.map(r=>`<details><summary>${esc(fieldLabels[r.command]??r.command)} · ${date(r.observedMs)} ET</summary>${content(r.value)}</details>`).join(''):'Sin resultados adicionales guardados en este lote.'}</section>
 <details class="panel"><summary>Cobertura de la captura · ${esc(view)}</summary>${content(saved?.coverage)}<p>Un dato guardado puede ser parcial o antiguo. Las fechas y estados originales se conservan. Esta captura no certifica todos los ticks del mercado.</p></details></details>`}</section>`;
}
export function bindSnapshotMode(render:()=>void){
 changed=render;
 document.querySelector<HTMLButtonElement>('[data-snapshot-toggle]')?.addEventListener('click',()=>{enabled=!enabled;changed();if(enabled)void poll();});
 document.querySelectorAll<HTMLButtonElement>('[data-snapshot-symbol]').forEach(b=>b.onclick=()=>{selected=b.dataset.snapshotSymbol!;changed();});
}
async function poll(){
 if(!enabled||busy||!isTauri())return;busy=true;
 try {const result=await invoke<SavedBundle>('get_snapshot_bundle');
  if(result.available&&!validSavedBundle(result,Date.now()))throw Error('La captura no cumple la demora o identidad requerida.');
  if(result.available)saved=result;error=result.available?'':'Todavía no hay una captura elegible.';
 }catch{error='No se pudo consultar la base de datos. Se conserva la última captura disponible.';}
 finally{busy=false;if(enabled)changed();}
}
setInterval(()=>{void poll();},15000);
