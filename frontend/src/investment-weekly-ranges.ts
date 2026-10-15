import catalogue from '../../fixtures/legacy-universe.json';
import {weeklyCompanies,weeklyUniverseDate} from './weekly-company-universe';
import {invoke,isTauri} from './snapshot-observation';
import type {EtfReport} from './etf-dealers';
import {escapeHtml as esc} from './model';
import './investment-weekly-ranges.css';
export function weeklyRange(price:number,ivPercent:number,days:number){
 if(![price,ivPercent,days].every(Number.isFinite)||price<=0||ivPercent<=0||ivPercent>1000||days<=0||days>7)return null;
 const move=price*(ivPercent/100)*Math.sqrt(days/365);
 if(!Number.isFinite(move)||move>=price)return null;
 return {move,percent:move/price*100,lower:price-move,upper:price+move};
}
type Entry={symbol:string;price:number;iv:number;days:number;reference:string;expiry:string;source?:string};
let entries:Entry[]=[];
let requestedSymbols:string[]=[],symbolErrors=new Map<string,string>();
const unavailableCard=(symbol:string)=>`<article class="weekly-range-card weekly-range-empty"><header><h2>${esc(symbol)}</h2><span>${symbolErrors.has(symbol)?'Sin rango':'Consultando…'}</span></header><div class="weekly-range-levels">${[['weekly-lower','Inferior'],['weekly-base','Referencia'],['weekly-upper','Superior']].map(([c,l])=>`<div class="${c}"><span>${l}</span><b>—</b><i></i></div>`).join('')}</div><p>${esc(symbolErrors.get(symbol)??'Obteniendo precio e IV de Massive…')}</p></article>`;
const money=(n:number)=>'$'+n.toFixed(2);
function entryCards(values=entries){return values.length?values.map(e=>{const r=weeklyRange(e.price,e.iv,e.days)!;return `<article class="weekly-range-card"><header><h2>${esc(e.symbol)}</h2><span>Vencimiento ${esc(e.expiry.replace('T',' · '))} ET</span></header><p class="weekly-company-name">${esc(weeklyCompanies.find(c=>c.symbol===e.symbol)?.name??weeklyWatchlist.find(c=>c.symbol===e.symbol)?.name??e.symbol)}</p><div class="weekly-range-meta"><span>IV anualizada: ${e.iv.toFixed(2)}%</span><strong>Movimiento: ±${money(r.move)} <small>(${r.percent.toFixed(2)}%)</small></strong></div><div class="weekly-range-levels"><div class="weekly-lower"><span>Límite inferior</span><b>${money(r.lower)}</b><i></i><small>Extremo inferior estimado</small></div><div class="weekly-base"><span>Referencia</span><b>${money(e.price)}</b><i></i><small>Precio base</small></div><div class="weekly-upper"><span>Límite superior</span><b>${money(r.upper)}</b><i></i><small>Extremo superior estimado</small></div></div><footer>Referencia ${esc(e.reference.replace('T',' · '))} ET · ${e.days.toFixed(2)} días calendario · ${esc(e.source??'Datos introducidos manualmente')}</footer></article>`;}).join(''):`<article class="weekly-range-card weekly-range-empty"><header><h2>Rango semanal</h2><span>Pendiente de datos</span></header><div class="weekly-range-meta"><span>IV: —</span><strong>Movimiento: —</strong></div><div class="weekly-range-levels">${[['weekly-lower','Límite inferior'],['weekly-base','Referencia'],['weekly-upper','Límite superior']].map(([c,l])=>`<div class="${c}"><span>${l}</span><b>—</b><i></i></div>`).join('')}</div><p>Selecciona los activos y pulsa Escanear para obtener precio e IV de Massive y calcular los rangos disponibles.</p></article>`;}
const sectorLabel=(s:string)=>({TECNOLOGIA:'Tecnología',CONSUMO_DISCRECIONAL:'Consumo discrecional',COMUNICACION:'Comunicación',FINANCIERO:'Finanzas',SALUD:'Salud',ENERGIA:'Energía',CONSUMO_BASICO:'Consumo básico',INDUSTRIAL:'Industriales',MATERIALES:'Materiales',SERVICIOS_PUBLICOS:'Servicios públicos',INMOBILIARIO:'Inmobiliario'}[s]??s);
export const weeklyWatchlist=Object.entries(catalogue.assets).map(([symbol,a])=>({symbol,name:a.name,kind:a.kind}));
let scanMode='symbols',scanSector='all',resultGrouped=false;
export function weeklyScanSymbols(mode:string,sector:string,input:string){return mode==='watchlist'?weeklyWatchlist.map(a=>a.symbol):mode==='sectors'?weeklyCompanies.filter(c=>sector==='all'||c.sector===sector).map(c=>c.symbol):[...new Set(input.toUpperCase().split(/[\s,;]+/).filter(Boolean))];}
function cards(){
 if(!requestedSymbols.length)return entryCards();
 const render=(symbol:string)=>{const entry=entries.find(e=>e.symbol===symbol);return entry?entryCards([entry]):unavailableCard(symbol);};
 if(!resultGrouped)return requestedSymbols.map(render).join('');
 const groups=new Map<string,string[]>();for(const symbol of requestedSymbols){const sector=weeklyCompanies.find(c=>c.symbol===symbol)?.sector??'Mis activos';groups.set(sector,[...(groups.get(sector)??[]),symbol]);}
 return [...groups].map(([sector,symbols])=>`<section class="weekly-sector-group"><h3>${esc(sectorLabel(sector))} <small>${symbols.length} compañías</small></h3><div class="weekly-sector-grid">${symbols.map(render).join('')}</div></section>`).join('');
}
let scanSymbols='NVDA, SPY, QQQ',scanBusy=false,scanMessage='';
export function weeklyRangesPanel(){return `<section class="investment-weekly-ranges"><form data-weekly-scan class="ir-controls"><label>Buscar por<select name="mode"><option value="symbols" ${scanMode==='symbols'?'selected':''}>Mis tickers</option><option value="watchlist" ${scanMode==='watchlist'?'selected':''}>Todo el watchlist (${weeklyWatchlist.length} activos)</option><option value="sectors" ${scanMode==='sectors'?'selected':''}>Grandes compañías por sector</option></select></label><label>Sector<select name="sector" ${scanMode!=='sectors'?'disabled':''}><option value="all">Todos los sectores</option>${[...new Set(weeklyCompanies.map(c=>c.sector))].map(s=>`<option value="${s}" ${scanSector===s?'selected':''}>${sectorLabel(s)}</option>`).join('')}</select></label><label>Activos del scanner<input name="symbols" ${scanMode!=='symbols'?'disabled':''} value="${esc(scanSymbols)}" placeholder="NVDA, SPY, QQQ" required maxlength="280"></label><button type="submit" ${scanBusy||!isTauri()?'disabled':''}>${scanBusy?'Escaneando…':'Escanear rangos semanales'}</button></form><p class="weekly-universe-note">Selección de ${weeklyCompanies.length} grandes compañías de SPY, incluidas líderes del QQQ, agrupadas por el sector del motor. Posiciones de SPY al ${weeklyUniverseDate}; no es un ranking en vivo. <a href="https://www.ssga.com/us/en/intermediary/etfs/state-street-spdr-sp-500-etf-trust-spy" target="_blank" rel="noopener noreferrer">Fuente</a></p><p data-weekly-error role="status">${esc(scanMessage||(!isTauri()?'Abre la aplicación Windows para consultar Massive.':'El scanner obtiene precio, IV ATM y vencimiento de Massive.'))}</p><div data-weekly-cards>${cards()}</div><details class="ir-method"><summary>Fórmula y lectura del rango</summary><p>EM = P × IV × √(D / 365). IV anualizada decimal: promedio CALL y PUT del mismo strike ATM y vencimiento. D = días calendario desde el precio de referencia hasta el cierre de la sesión del vencimiento, incluyendo fracciones. Horizonte máximo: 7 días. Límites = P ± EM. Convención ACT/365; cierre ET del calendario del mercado, incluidos cierres anticipados.</p><p>El scanner elige el último vencimiento disponible dentro de 7 días: normalmente el viernes. Exige precio y cotizaciones recientes, IV en ambos lados, strike ATM a menos de 2% del precio y spread máximo de 20%. Si faltan datos verificables no calcula un rango. Fuera de sesión utiliza el cierre de la última sesión y cotizaciones ATM del mismo día, a un máximo de 15 minutos del cierre y no posteriores a más de 2 minutos de ese cierre. Se marca como referencia histórica; si el proveedor no conserva IV y cotizaciones compatibles, queda pendiente.</p><p>Rango aproximado, sin dirección ni probabilidades calibradas. No son soportes confirmados. Actualización automática cada minuto mientras este panel está visible, después del primer escaneo. Los fallos se vuelven a consultar; cada rango exige datos válidos.</p></details></section>`;}
// ET has the same DST switch dates throughout the supported contemporary years.
export function easternTimestamp(value:string){
 if(!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(value))return NaN;
 const base=Date.parse(value+'Z');if(!Number.isFinite(base))return NaN;
 const fmt=new Intl.DateTimeFormat('sv-SE',{timeZone:'America/New_York',year:'numeric',month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit',hourCycle:'h23'});
 const matches=[base+4*3600000,base+5*3600000].filter(t=>fmt.format(new Date(t)).replace(' ','T')===value);
 return matches.length===1?matches[0]:NaN;
}
const etText=(ms:number)=>new Intl.DateTimeFormat('sv-SE',{timeZone:'America/New_York',year:'numeric',month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit',hourCycle:'h23'}).format(ms).replace(' ','T');
export type WeeklySnapshot=EtfReport&{expirationCloses?:Record<string,number>;historicalSession?:{openMs:number;closeMs:number;date:string}|null};
export function scannedWeeklyEntry(r:WeeklySnapshot,symbol:string,now:number):Entry{
 const ds=r.enrichment?.details,session=r.historicalSession;
 const historical=!!session&&Number.isFinite(session.openMs)&&Number.isFinite(session.closeMs)&&session.openMs<session.closeMs&&session.closeMs<=now&&now-session.closeMs<=7*86400000&&r.priceAt===session.closeMs;
 if(session&&!historical)throw Error('Referencia histórica inválida.');
 if(r.symbol!==symbol||r.days!==7)throw Error('Respuesta de Massive incompatible con el activo o el horizonte solicitado.');
 if(!Number.isFinite(r.at)||r.at>now||now-r.at>300000)throw Error('Consulta de Massive caducada o con fecha inválida.');
 if(!Number.isFinite(r.spot)||r.spot<=0)throw Error('Precio de Massive ausente o inválido.');
 if(!r.priceAt||!Number.isFinite(r.priceAt)||r.priceAt>now)throw Error('Fecha del precio ausente, inválida o adelantada.');
 if(!historical&&now-r.priceAt>120000)throw Error('Precio de Massive caducado: supera 2 minutos.');
 if(!ds?.length)throw Error('Cadena de opciones de Massive vacía.');
 const expires=(date:string)=>r.expirationCloses?Object.prototype.hasOwnProperty.call(r.expirationCloses,date)?r.expirationCloses[date]:NaN:easternTimestamp(date+'T16:00');
 const dates=[...new Set(ds.map(c=>c.expiration))].filter(d=>/^\d{4}-\d{2}-\d{2}$/.test(d)&&Number.isFinite(expires(d))&&expires(d)>now&&expires(d)-r.priceAt!<=7*86400000).sort();
 const expiration=dates.at(-1);if(!expiration)throw Error('Sin vencimiento semanal disponible.');
 const rows=ds.filter(c=>c.expiration===expiration&&Number.isFinite(c.strike)&&c.strike>0);
 const strike=[...new Set(rows.map(c=>c.strike))].sort((a,b)=>Math.abs(a-r.spot)-Math.abs(b-r.spot)||a-b)[0];
 if(!strike||Math.abs(strike/r.spot-1)>.02)throw Error('Sin strike cercano al dinero.');
 const sides=['call','put'].map(kind=>rows.filter(c=>c.strike===strike&&c.kind===kind));
 if(sides.some(s=>s.length!==1))throw Error('Par ATM CALL/PUT ausente o ambiguo.');
 const pair=sides.map(s=>s[0]);
 for(const c of pair){
  const side=c.kind.toUpperCase();
  if(!c.realtime)throw Error(`${side}: cotización no marcada en tiempo real por el proveedor.`);
  if(c.iv==null||!Number.isFinite(c.iv)||c.iv<=0||c.iv>10)throw Error(`${side}: IV ausente o inválida en strike ${strike.toFixed(2)}.`);
  if(!c.quoteAt||!Number.isFinite(c.quoteAt)||c.quoteAt>now)throw Error(`${side}: fecha de cotización ausente o adelantada.`);
  if(historical?(c.quoteAt<session!.openMs||c.quoteAt>session!.closeMs+120000||Math.abs(c.quoteAt-session!.closeMs)>15*60000):(now-c.quoteAt>120000||Math.abs(c.quoteAt-r.priceAt!)>120000))throw Error(`${side}: cotización incompatible con la fecha del precio de referencia.`);
  if(c.bid==null||c.ask==null||![c.bid,c.ask].every(Number.isFinite)||c.bid<=0||c.ask<c.bid)throw Error(`${side}: bid/ask ausentes o inválidos.`);
  const spread=100*(c.ask-c.bid)/((c.ask+c.bid)/2);if(spread>20)throw Error(`${side}: spread ${spread.toFixed(1)}%, superior al 20%.`);
 }
 const iv=(pair[0].iv!+pair[1].iv!)/2*100,expiry=etText(expires(expiration)),days=(expires(expiration)-r.priceAt)/86400000;
 if(!weeklyRange(r.spot,iv,days))throw Error('Rango semanal inválido.');
 return {symbol,price:r.spot,iv,days,reference:etText(r.priceAt),expiry,source:`${historical?'Referencia histórica · última sesión · ':'Datos recientes · '}Massive · IV ATM CALL/PUT · strike ${strike.toFixed(2)} · consulta ${etText(r.at)} ET`};
}
function paintWeeklyScan(){const panel=document.querySelector<HTMLElement>('.investment-weekly-ranges');if(!panel)return;const status=panel.querySelector('[data-weekly-error]'),host=panel.querySelector('[data-weekly-cards]'),button=panel.querySelector<HTMLButtonElement>('button[type="submit"]');if(status)status.textContent=scanMessage;if(host)host.innerHTML=cards();if(button){button.disabled=scanBusy||!isTauri();button.textContent=scanBusy?'Escaneando…':'Escanear rangos semanales';}}
export function bindWeeklyRanges(root:HTMLElement){const form=root.querySelector<HTMLFormElement>('[data-weekly-scan]');if(!form)return;
 const mode=form.querySelector<HTMLSelectElement>('[name="mode"]')!,sector=form.querySelector<HTMLSelectElement>('[name="sector"]')!,input=form.querySelector<HTMLInputElement>('[name="symbols"]')!;
 mode.onchange=()=>{scanMode=mode.value;sector.disabled=scanMode!=='sectors';input.disabled=scanMode!=='symbols';};sector.onchange=()=>{scanSector=sector.value;};
 form.onsubmit=async e=>{
 e.preventDefault();if(scanBusy||!isTauri())return;
 const symbols=weeklyScanSymbols(mode.value,sector.value,input.value);
 const status=root.querySelector('[data-weekly-error]')!,host=root.querySelector('[data-weekly-cards]')!;
 if(!symbols.length||symbols.length>(mode.value==='watchlist'?weeklyWatchlist.length:mode.value==='sectors'?50:20)||symbols.some(s=>!/^([A-Z][A-Z0-9.\-]{0,11})$/.test(s))){status.textContent='Introduce hasta 20 símbolos válidos separados por comas.';return;}
 if(mode.value==='symbols')scanSymbols=symbols.join(', ');scanMode=mode.value;scanSector=sector.value;resultGrouped=scanMode==='sectors';scanBusy=true;scanMessage='Consultando Massive…';status.textContent=scanMessage;form.querySelector('button')!.disabled=true;entries=[];requestedSymbols=symbols;symbolErrors.clear();host.innerHTML=cards();
 await refreshWeeklyScan(symbols);
};}
let refreshTimer:ReturnType<typeof setTimeout>|undefined;
function scheduleWeeklyRefresh(){
 if(refreshTimer!==undefined)clearTimeout(refreshTimer);
 refreshTimer=setTimeout(async()=>{
  if(!scanBusy&&requestedSymbols.length&&isTauri()&&document.querySelector('.investment-weekly-ranges')&&document.visibilityState!=='hidden')await refreshWeeklyScan([...requestedSymbols]);
  else scheduleWeeklyRefresh();
 },60000);
}
async function refreshWeeklyScan(symbols:string[]){
 scanBusy=true;entries=[];symbolErrors.clear();scanMessage='Actualizando precio y opciones de Massive…';paintWeeklyScan();
 const errors:string[]=[];
 try{let cursor=0;const worker=async()=>{while(cursor<symbols.length){const symbol=symbols[cursor++];scanMessage=`Escaneando ${symbol}…`;paintWeeklyScan();try{if(weeklyWatchlist.find(a=>a.symbol===symbol)?.kind==='INDEX')throw Error('Índice: rango pendiente de integración de precio y opciones de índices.');const report=await invoke<WeeklySnapshot>('get_weekly_range_snapshot',{symbol});entries.push(scannedWeeklyEntry(report,symbol,Date.now()));}catch(e){symbolErrors.set(symbol,String(e));errors.push(`${symbol}: ${String(e)}`);}paintWeeklyScan();}};await Promise.all(Array.from({length:Math.min(3,symbols.length)},worker));
 scanMessage=`${entries.length} / ${symbols.length} rangos calculados · ${errors.length} pendientes. Revisa el motivo en cada tarjeta.`;
 }finally{scanBusy=false;paintWeeklyScan();scheduleWeeklyRefresh();}
}
