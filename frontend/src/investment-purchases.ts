import {bundledAssets} from './asset-watchlist';
import {blockAlertHistory} from './block-alerts-ui';
import {invoke,isTauri} from './snapshot-observation';
import {escapeHtml as esc} from './model';
import './investment-purchases.css';
import type {Block} from './investment-research';
export interface Purchases {date:string;throughMs:number;rows:{ticker:string;block:Block;execution?:Record<string,unknown>}[];coverage:string[]}
let report:Purchases|null=null,busy=false,error='',symbols='AAPL, MSFT, NVDA, AMZN, META',minimum=10000000,maximum=0,sideFilter='all',scope='workspace',progress='',stopRequested=false,scanned=0,total=0,searchedMinimum=0;
let previousReport:Purchases|null=null,previousForDate='',previousStatus='';
let historicalReport:Purchases|null=null,historicalDate='',historicalError='',restoredDate='',activeWindow='';
const newYorkDate=(at=Date.now())=>new Date(at).toLocaleDateString('sv-SE',{timeZone:'America/New_York'});
const newYorkHour=(at=Date.now())=>Number(new Intl.DateTimeFormat('en-US',{timeZone:'America/New_York',hour:'2-digit',hourCycle:'h23'}).format(at));
const windowKey=(at=Date.now())=>newYorkDate(at)+'|'+(newYorkHour(at)<16?'morning':'afternoon');
const windowStart=(r:Purchases)=>newYorkHour(r.throughMs)<16?'04:00':'16:00';
const priorDate=()=>newYorkDate(Date.now()-86400000);
const defaultBlockUniverse=()=>[...new Set([...Object.entries(bundledAssets).filter(([,asset])=>asset.kind!=='INDEX').map(([ticker])=>ticker),'SMH'])];
export const concreteBlockSymbols=(input:string)=>[...new Set(input.toUpperCase().split(/[,;\s]+/).filter(Boolean).map(symbol=>symbol==='GOOGLE'||symbol==='ALPHABET'?'GOOGL':symbol))];
const money=(v:number)=>new Intl.NumberFormat('en-US',{style:'currency',currency:'USD',maximumFractionDigits:2}).format(v);
const time=(v:number)=>new Date(v).toLocaleTimeString('es',{timeZone:'America/New_York',hourCycle:'h23',hour:'2-digit',minute:'2-digit',second:'2-digit'});
export function blockSide(b:Block):'Compra estimada'|'Venta estimada'|'Sin clasificar' {
 if(b.bid==null||b.ask==null||b.quoteMs==null||b.executionMs==null||b.bid<=0||b.ask<=b.bid||b.quoteMs>b.executionMs||b.executionMs-b.quoteMs>2000||(b.ask-b.bid)/((b.ask+b.bid)/2)>0.01)return 'Sin clasificar';
 if(b.conditions.some(condition=>![0,12,37].includes(condition)))return 'Sin clasificar';
 if(b.nbbo==='En ask'||b.nbbo==='Sobre ask')return 'Compra estimada';
 if(b.nbbo==='En bid'||b.nbbo==='Bajo bid')return 'Venta estimada';
 return 'Sin clasificar';
}
export function nbboPosition(b:Block):string {
 if(b.bid==null||b.ask==null||b.quoteMs==null||b.executionMs==null)return b.nbbo||'Sin cotización NBBO válida';
 if(b.bid<=0||b.ask<=b.bid)return 'Cotización NBBO inválida';
 if(b.quoteMs>b.executionMs||b.executionMs-b.quoteMs>2000)return 'Cotización NBBO fuera de tiempo';
 if((b.ask-b.bid)/((b.ask+b.bid)/2)>0.01)return 'Spread NBBO demasiado amplio';
 return ({'Sobre ask':'Above Ask','En ask':'At Ask','Bajo bid':'Below Bid','En bid':'At Bid','Entre bid y ask':'Between Bid / Ask'} as Record<string,string>)[b.nbbo]??b.nbbo;
}
export function blockCard(ticker:string,b:Block,execution?:Record<string,unknown>,previous?:Purchases["rows"][number],previousDate=""):string {
 const special=b.conditions.some(condition=>![0,12,37].includes(condition)),side=blockSide(b),position=nbboPosition(b),tone=side==='Compra estimada'?'buy':side==='Venta estimada'?'sell':'unknown';
 const date=b.executionMs?new Date(b.executionMs).toLocaleDateString('es',{timeZone:'America/New_York',day:'numeric',month:'long',year:'numeric'}):'Fecha no verificable';
 const prior=previous?.block;
 const prev=(value:string)=>prior?`<div class="ir-block-previous"><span>Sesión previa · ${esc(previousDate)}</span><strong>${value}</strong></div>`:"";
 return `<article class="ir-block-card ${tone}"><header><span class="ir-block-source">DATA <small>TRF · fuera de bolsa</small></span><span class="ir-block-side">${side}</span></header><h3>Bloque: ${esc(ticker)}</h3><div class="ir-block-facts"><div><span>Hora de ejecución</span><strong>${b.executionMs?time(b.executionMs)+' ET':'No verificable'}</strong><small>${esc(date)}</small>${prev(prior?.executionMs?time(prior.executionMs)+" ET":"Hora no verificable")}</div><div><span>Acciones @ Precio</span><strong>${b.shares.toLocaleString('en-US')} @ ${money(b.price)}</strong>${prev(prior?prior.shares.toLocaleString("en-US")+" @ "+money(prior.price):"")}</div><div><span>Importe ejecutado</span><strong>${money(b.notional)}</strong><small>(${(b.notional/1000000).toFixed(2)}M)</small>${prev(prior?money(prior.notional):"")}</div></div><div class="ir-block-nbbo"><span>Posición frente al NBBO</span><strong>${position}${special?' · condición especial':''} <i class="${position.includes('Ask')&&!position.startsWith('Between')?'buy':position.includes('Bid')&&!position.startsWith('Between')?'sell':'unknown'}"></i></strong><small>Bid ${b.bid==null?'—':money(b.bid)} · Ask ${b.ask==null?'—':money(b.ask)}</small></div><footer>Datos de Data · ID ${esc(b.id??'—')} · TRF ${b.trfId}</footer><details><summary>Información completa de ejecución</summary><p>SIP: ${time(b.sipMs)} ET · TRF: ${b.trfMs?time(b.trfMs)+' ET':'—'}</p><p>NBBO: ${b.quoteMs?time(b.quoteMs)+' ET':'No verificable'} · antigüedad ${b.quoteMs!=null&&b.executionMs!=null?b.executionMs-b.quoteMs+' ms':'—'}</p><p>Condiciones: ${esc(b.conditions.join(', ')||'Sin códigos recibidos')} · ${esc(b.nbbo)} · Short: no identificable con este reporte</p><pre>${esc(JSON.stringify(execution??{},null,2))}</pre>${prior?`<p>Sesión previa ${esc(previousDate)} · ID ${esc(prior.id??"—")} · TRF ${prior.trfId} · ${esc(nbboPosition(prior))} · ${esc(blockSide(prior))}</p><pre>${esc(JSON.stringify(previous?.execution??{},null,2))}</pre>`:""}</details></article>`;
}
export function filteredBlocks(r:Purchases,minimumAmount=0,maximumAmount=0,side='all') {
 return r.rows.filter(v=>v.block.notional>=minimumAmount&&(!maximumAmount||v.block.notional<=maximumAmount)&&(side==='all'||blockSide(v.block)===(side==='buy'?'Compra estimada':side==='sell'?'Venta estimada':'Sin clasificar'))).sort((a,b)=>b.block.notional-a.block.notional);
}
export function purchasesResult(r:Purchases|null,message='',filter?:{minimum:number;maximum:number;side:string},previous:Purchases|null=null):string {
 const rows=r?(filter?filteredBlocks(r,filter.minimum,filter.maximum,filter.side):r.rows):[];
 const buys=rows.filter(v=>blockSide(v.block)==='Compra estimada'),sells=rows.filter(v=>blockSide(v.block)==='Venta estimada');
 const used=new Map<string,number>();
 const previousRows=previous&&r&&previous.date<r.date?filteredBlocks(previous):[];
 const previousCard=(ticker:string,block:Block,execution?:Record<string,unknown>)=>{
  const key=ticker+':'+blockSide(block),index=used.get(key)??0;used.set(key,index+1);
  const match=previousRows.filter(row=>row.ticker===ticker&&blockSide(row.block)===blockSide(block))[index];
  return blockCard(ticker,block,execution,match,previous?.date);
 };
 if(!r)return `${message?`<p class="ir-error">${esc(message)}</p>`:''}<p class="ir-footnote">Los bloques guardados aparecerán aquí en tarjetas.</p>`;
 const sum=(a:typeof rows)=>money(a.reduce((total,v)=>total+v.block.notional,0));
 const companies=[...new Set(rows.map(v=>v.ticker))].map(ticker=>{const blocks=rows.filter(v=>v.ticker===ticker);return {ticker,count:blocks.length,total:blocks.reduce((n,v)=>n+v.block.notional,0),largest:Math.max(...blocks.map(v=>v.block.notional))};}).sort((a,b)=>b.total-a.total);
 const cards=rows.map(({ticker,block,execution})=>previousCard(ticker,block,execution)).join('');
 const summary=`<p>${esc(r.date)} · ${windowStart(r)}–${time(r.throughMs)} ET · ${r.rows.length} bloques encontrados · ${rows.length} visibles${r.rows.length>rows.length?' · Ajusta los filtros para ver los demás':''}</p><div class="ir-kpis">${[['BLOQUES',rows.length,sum(rows)],['COMPRAS ESTIMADAS',buys.length,sum(buys)],['VENTAS ESTIMADAS',sells.length,sum(sells)],['SIN CLASIFICAR',rows.length-buys.length-sells.length,'Dirección no verificable']].map(([label,count,amount])=>`<article><span>${label}</span><strong>${count}</strong><small>${amount}</small></article>`).join('')}</div>${companies.length?`<div class="ir-table-wrap"><table><thead><tr><th>Activo</th><th>Bloques que cumplen</th><th>Importe total</th><th>Mayor bloque</th></tr></thead><tbody>${companies.map(a=>`<tr><td><b>${esc(a.ticker)}</b></td><td>${a.count}</td><td>${money(a.total)}</td><td>${money(a.largest)}</td></tr>`).join('')}</tbody></table></div>`:''}<details class="ir-purchases-coverage"><summary>Cobertura de la búsqueda</summary>${r.coverage.map(c=>`<p>${esc(c)}</p>`).join('')}</details>`;
 return `${message?`<p class="ir-error">${esc(message)}</p>`:''}${cards?`<div class="ir-block-feed">${cards}</div>`:'<p class="ir-footnote">Sin bloques del importe solicitado en la muestra consultada.</p>'}<details class="ir-purchases-summary"><summary>Ver resumen por compañía y cobertura</summary>${summary}</details>`;
}export function purchasesPanel():string{return `<section class="ir-daily-purchases ir-list"><header><div><span class="ir-eyebrow">ACCIONES Y ETF · OPERACIONES FUERA DE BOLSA</span><h2>Buscar bloques grandes · hoy</h2></div></header><form data-ir-purchases-form class="ir-controls"><label>Buscar en<select name="scope">${[['workspace',`Workspace + SMH (${defaultBlockUniverse().length})`],['market','Todo el mercado disponible en Data'],['symbols','Símbolos concretos']].map(([v,l])=>`<option value="${v}" ${scope===v?'selected':''}>${l}</option>`).join('')}</select></label><label>Símbolos · solo búsqueda concreta<input name="symbols" value="${esc(symbols)}" maxlength="150" placeholder="AAPL, MSFT, NVDA…"></label><label>Importe mínimo por bloque · USD<input name="minimum" type="number" min="100000" max="1000000000" step="100000" value="${minimum}" required list="ir-block-amounts"><datalist id="ir-block-amounts">${[1000000,5000000,10000000,25000000,50000000,100000000].map(n=>`<option value="${n}">${money(n)}</option>`).join('')}</datalist></label><label>Importe máximo · opcional<input name="maximum" type="number" min="0" step="100000" value="${maximum||''}" placeholder="Sin límite"></label><label>Mostrar<select name="side">${[['buy','Compras estimadas'],['sell','Ventas estimadas'],['all','Todos los bloques'],['unknown','Sin clasificar']].map(([v,l])=>`<option value="${v}" ${sideFilter===v?'selected':''}>${l}</option>`).join('')}</select></label><button data-ir-purchases-submit ${busy||!isTauri()?'disabled':''}>${busy?'Buscando…':'Buscar bloques de hoy'}</button><button type="button" data-ir-purchases-stop ${busy?'':'disabled'}>Detener búsqueda</button></form><details class="ir-purchases-status"><summary>Estado de búsqueda</summary><p data-ir-purchases-progress role="status">${esc(progress)}</p><p data-ir-previous-status role="status">${esc(previousStatus)}</p></details><div data-ir-purchases-result>${purchasesResult(report,error||(report&&searchedMinimum&&minimum<searchedMinimum?'Pulsa Buscar para consultar importes inferiores al mínimo de la búsqueda anterior.':'') ,{minimum,maximum,side:sideFilter},previousReport)}</div>${historySection()}${blockAlertHistory()}<p class="ir-footnote">Importes por ejecución individual, no suma de operaciones pequeñas. Resultados agrupados por activo y tarjetas ordenadas por importe. Compra/venta estimada contra NBBO; no identifica shorts. Workspace = universo configurado. Mercado = símbolos activos de acciones y ETF de EE. UU. disponibles en Data, incluyendo otros valores del mercado de acciones. La búsqueda avanza por activo, con resultados parciales hasta terminar. Detener conserva los recibidos. Histórico máximo 1.000.000 operaciones por activo y NBBO hasta 200 bloques; cualquier falta de cobertura se indica. Nueva York · 04:00–16:00 ET para la mañana; 16:00–20:00 ET para la tarde. La mañana se muestra hasta las 16:00; la tarde se conserva para comparar mañana.</p></section>`;}
export function bindPurchases(root:HTMLElement,available:()=>boolean,onBusy:(v:boolean)=>void){
 const form=root.querySelector<HTMLFormElement>('[data-ir-purchases-form]');if(!form)return;
 const day=newYorkDate();
 async function loadPrevious(date:string){
  previousForDate=date;previousReport=null;previousStatus='Consultando sesión previa guardada…';
  for(let offset=1;offset<=7;offset++){
   const prior=newYorkDate(Date.parse(date+'T12:00:00Z')-offset*86400000);
   try{const saved=await invoke<{minimum:number;report:Purchases}|null>('get_saved_daily_purchases',{date:prior});
    if(previousForDate!==date)return;
    if(saved?.report.date===prior){previousReport=saved.report;previousStatus='Sesión previa guardada: '+prior;draw();return;}
   }catch(e){previousStatus='No se pudo leer la sesión previa: '+String(e);draw();return;}
  }
  previousStatus='Sin sesión previa guardada en los últimos 7 días';draw();
 }
 if(previousForDate!==day)void loadPrevious(day);
 if(restoredDate!==windowKey()){
  restoredDate=windowKey();
  if(report?.date!==day||activeWindow!==windowKey()){activeWindow=windowKey();if(report&&report.date!==day){historicalReport=report;historicalDate=report.date;}report=null;progress='';}
  void invoke<{minimum:number;report:Purchases}|null>('get_saved_daily_purchases',{date:day}).then(saved=>{
   if(!busy&&report===null&&saved?.report.date===day){report=saved.report;searchedMinimum=saved.minimum;minimum=saved.minimum;const input=form.querySelector<HTMLInputElement>('[name="minimum"]');if(input)input.value=String(minimum);progress=`Bloques recuperados del ${day} · ${report.rows.length} operaciones · actualiza la búsqueda para incorporar nuevas`;draw();}
  }).catch(e=>{error=`No se pudieron recuperar los bloques guardados: ${String(e)}`;draw();});
  if(!historicalDate){
   historicalDate=priorDate();
   const previous=historicalDate;
   void invoke<{minimum:number;report:Purchases}|null>('get_saved_daily_purchases',{date:previous}).then(saved=>{if(saved&&!historicalReport&&historicalDate===previous){historicalReport=saved.report;draw();}}).catch(()=>{/* An absent prior session does not affect today's search. */});
  }
 }
 const historyButton=root.querySelector<HTMLButtonElement>('[data-ir-purchases-history-load]');
 if(historyButton)historyButton.onclick=async()=>{
  const input=root.querySelector<HTMLInputElement>('[data-ir-purchases-history-date]');const date=input?.value??'';
  if(!/^\d{4}-\d{2}-\d{2}$/.test(date)||date>newYorkDate()){historicalError='Elige una fecha válida hasta hoy';draw();return;}
  historicalDate=date;historicalError='Consultando bloques guardados…';draw();
  try{const saved=await invoke<{minimum:number;report:Purchases}|null>('get_saved_daily_purchases',{date});historicalReport=saved?.report??null;historicalError=saved?'':'No hay una búsqueda guardada para esa fecha';}
  catch(e){historicalReport=null;historicalError=`No se pudieron leer los bloques guardados: ${String(e)}`;}
  draw();
 };
 const read=()=>{const d=new FormData(form);minimum=Number(d.get('minimum'));maximum=Number(d.get('maximum')||0);sideFilter=String(d.get('side')||'all');scope=String(d.get('scope')||'workspace');symbols=String(d.get('symbols')||'').trim().toUpperCase();};
 const symbolInput=form.querySelector<HTMLInputElement>('[name="symbols"]');
 if(symbolInput)symbolInput.oninput=()=>{const select=form.querySelector<HTMLSelectElement>('[name="scope"]');if(select)select.value='symbols';read();};
 form.onchange=()=>{read();draw();};
 const stop=root.querySelector<HTMLButtonElement>('[data-ir-purchases-stop]');if(stop)stop.onclick=()=>{stopRequested=true;progress='Deteniendo al terminar el activo en curso…';draw();};
 form.onsubmit=async e=>{e.preventDefault();if(busy||!isTauri()||!available())return;read();if(!Number.isFinite(minimum)||minimum<100000||minimum>1000000000||!Number.isFinite(maximum)||maximum<0||(maximum>0&&maximum<minimum)){error='Revisa el rango de importes: mínimo desde $100.000 y máximo mayor o igual al mínimo';draw();return;}
 if(newYorkHour()<4){error='La búsqueda de hoy empieza a las 04:00 ET. Puedes consultar los bloques guardados del día anterior.';progress='';draw();return;}
 if(activeWindow!==windowKey()){activeWindow=windowKey();report=null;restoredDate='';} const queryMinimum=minimum,queryScope=scope,querySymbols=symbols;searchedMinimum=queryMinimum;
 void loadPrevious(newYorkDate());
 busy=true;onBusy(true);error='';stopRequested=false;scanned=0;total=0;progress=report?.date===newYorkDate()?'Mostrando bloques guardados mientras se actualiza la búsqueda…':'Cargando bloques guardados…';draw();
 try{
  if(report?.date!==newYorkDate())report=null;
  if(!report){try{const saved=await invoke<{minimum:number;report:Purchases}|null>('get_saved_daily_purchases',{date:newYorkDate()});if(saved?.report.date===newYorkDate()){report=saved.report;progress=`${report.rows.length} bloques guardados visibles · preparando actualización…`;draw();}}catch{/* Continue with the live search if local history is unavailable. */}}
  let tickers=queryScope==='market'?await invoke<string[]>('get_daily_purchases_universe'):queryScope==='workspace'?defaultBlockUniverse():concreteBlockSymbols(querySymbols);
  tickers=[...new Set(tickers)];if(!tickers.length||tickers.some(t=>!/^[A-Z0-9.-]{1,12}$/.test(t))||(queryScope==='symbols'&&tickers.length>10))throw new Error('Indica entre 1 y 10 símbolos válidos para la búsqueda concreta');
  if(queryScope==='symbols'&&report){const selected=new Set(tickers);report={...report,rows:report.rows.filter(row=>selected.has(row.ticker)),coverage:report.coverage.filter(line=>tickers.some(ticker=>line.startsWith(ticker+':')))};draw();}
  // The liquid workspace assets appear first in the market scan; all others remain included.
  if(queryScope==='market')tickers.sort((a,b)=>Number(!!bundledAssets[b])-Number(!!bundledAssets[a])||a.localeCompare(b));
  total=tickers.length;
  for(const ticker of tickers){if(stopRequested)break;progress=`${scanned} / ${total} activos revisados · consultando ${ticker} · resultados parciales`;draw();
   try{const next=await invoke<Purchases>('search_daily_purchases',{symbols:ticker,minimum:queryMinimum});if(report&&next.date!==report.date){stopRequested=true;error='Cambió la fecha de Nueva York durante la búsqueda. Conservamos el día anterior; inicia una nueva búsqueda para hoy.';break;}if(!report)report={...next,rows:[],coverage:[]};report.rows=report.rows.filter(row=>row.ticker!==ticker);report.coverage=report.coverage.filter(line=>!line.startsWith(ticker+':'));report.rows.push(...next.rows);report.coverage.push(...next.coverage);report.throughMs=Math.max(report.throughMs,next.throughMs);}catch(e){if(!report)report={date:newYorkDate(),throughMs:Date.now(),rows:[],coverage:[]};report.coverage.push(`${ticker}: error ${String(e)}`);}
   scanned++;draw();
  }
  progress=`${stopRequested?'Búsqueda detenida · muestra parcial':'Búsqueda terminada'} · ${scanned} / ${total} activos revisados · mínimo consultado ${money(queryMinimum)}`;
 }catch(e){error=String(e);progress='Búsqueda no completada';}finally{busy=false;onBusy(false);draw();}
 };
 function draw(){const active=document.querySelector('.investment-radar')??root;const result=active.querySelector('[data-ir-purchases-result]');if(result){const wasOpen=result.querySelector<HTMLDetailsElement>('.ir-purchases-summary')?.open??false;result.innerHTML=purchasesResult(report,error||(report&&searchedMinimum&&minimum<searchedMinimum?'Pulsa Buscar para consultar importes inferiores al mínimo de la búsqueda anterior.':'') ,{minimum,maximum,side:sideFilter},previousReport);const summaryDetails=result.querySelector<HTMLDetailsElement>('.ir-purchases-summary');if(summaryDetails)summaryDetails.open=wasOpen;}const history=active.querySelector('[data-ir-purchases-history-result]');if(history)history.innerHTML=historicalContent();const summary=active.querySelector('[data-ir-purchases-history-summary]');if(summary)summary.textContent=historicalSummary();const priorStatus=active.querySelector('[data-ir-previous-status]');if(priorStatus)priorStatus.textContent=previousStatus;const p=active.querySelector('[data-ir-purchases-progress]');if(p)p.textContent=progress;const b=active.querySelector<HTMLButtonElement>('[data-ir-purchases-submit]');if(b){b.disabled=busy||!isTauri();b.textContent=busy?'Buscando…':'Buscar bloques de hoy';}const stop=active.querySelector<HTMLButtonElement>('[data-ir-purchases-stop]');if(stop)stop.disabled=!busy;}
}
export function savedDayLabel(date:string,previousDate=priorDate()):string{return date===previousDate?'DÍA ANTERIOR':'DÍA GUARDADO';}
function historicalSummary():string{return historicalReport?`${savedDayLabel(historicalReport.date)} · ${historicalReport.date} · ${historicalReport.rows.length} bloques`:'Comparar con un día guardado';}
function historicalContent():string{return historicalError?`<p class="ir-error">${esc(historicalError)}</p>`:historicalReport?`<div class="ir-history-label">${savedDayLabel(historicalReport.date)} · ${esc(historicalReport.date)} · NUEVA YORK</div>${purchasesResult(historicalReport,'',{minimum:0,maximum:0,side:'all'})}`:'<p>Selecciona una fecha anterior para comparar compras y ventas.</p>';}
function historySection():string{return `<section class="ir-purchases-history" aria-label="Bloques guardados de sesiones anteriores"><h3 data-ir-purchases-history-summary>${historicalSummary()}</h3><label>Fecha de Nueva York <input data-ir-purchases-history-date type="date" max="${newYorkDate()}" value="${esc(historicalDate||priorDate())}"></label><button type="button" data-ir-purchases-history-load>Ver bloques guardados</button><div data-ir-purchases-history-result>${historicalContent()}</div></section>`;}
