import {invoke,isTauri} from './snapshot-observation';
import {bundledAssets} from './asset-watchlist';
import {escapeHtml as esc} from './model';
import './block-alerts-ui.css';

type Config={scope:{selected:string[]}|"watchlist";minimumNotional:number;alertBuy:boolean;alertSell:boolean};
type Alert={key:string;symbol:string;side:'BUY'|'SELL';executionMs:number;notional:number;price:number;shares:number;nbbo:string};
type Stored={alert:Alert;receivedMs:number;read:boolean};
type Status={automatic?:boolean;enabled:boolean;config:Config|null;scanned:number;total:number;lastSymbol:string;lastScanMs:number;error:string;unread:number;history:Stored[];morningWindow:boolean};
let current:Status|null=null;
let timer:number|undefined;
let loading=false;
let localError='';
const watchlist=Object.entries(bundledAssets).filter(([,asset])=>asset.kind!=='INDEX').map(([symbol])=>symbol).sort();
const money=(value:number)=>new Intl.NumberFormat('en-US',{style:'currency',currency:'USD',maximumFractionDigits:0}).format(value);
const time=(value:number)=>new Date(value).toLocaleString('es',{timeZone:'America/New_York',hour12:false});
const parseSymbols=(input:string)=>[...new Set(input.toUpperCase().split(/[,;\s]+/).filter(Boolean).map(symbol=>symbol==='GOOGLE'||symbol==='ALPHABET'?'GOOGL':symbol))];

export function blockAlertsPanel():string {
 const config=current?.config;
 const chosen=typeof config?.scope==='object'?config.scope.selected.join(', '):'QQQ, GOOGL';
 return `<section class="block-alert-panel" aria-label="Alarmas de bloques"><header><span class="ir-eyebrow">BLOQUES · ALARMAS</span><h2>Vigilar compras y ventas estimadas</h2><p>Búsqueda automática de 04:00 a 16:00 ET mientras Snipers está abierto. Recorre los activos por lotes; el watchlist completo tarda más que una lista corta.</p></header><form data-block-alert-form><label>Buscar en<select name="scope"><option value="selected" ${config?.scope!=='watchlist'?'selected':''}>Activos que elija</option><option value="watchlist" ${config?.scope==='watchlist'?'selected':''}>Watchlist completo · ${watchlist.length} activos</option></select></label><label>Activos manuales<input name="symbols" value="${esc(chosen)}" placeholder="QQQ, GOOGL, NVDA"></label><label>Importe mínimo por bloque<input name="minimum" type="number" min="100000" max="1000000000" step="100000" value="${config?.minimumNotional??10000000}"></label><label><input name="buy" type="checkbox" ${config?.alertBuy!==false?'checked':''}> Compras estimadas</label><label><input name="sell" type="checkbox" ${config?.alertSell!==false?'checked':''}> Ventas estimadas</label><button type="submit" data-block-alert-toggle>${current?.enabled?'Actualizar vigilancia':'Activar búsqueda automática'}</button><button type="button" data-block-alert-stop ${current?.enabled?'':'disabled'}>Detener</button></form><div data-block-alert-status>${statusMarkup()}</div><p class="block-alert-note">La vigilancia automática se reanuda cada día mientras Snipers está abierto. Detener desactiva también la reanudación automática. Las alarmas cuentan desde la activación y se buscan de 04:00 a 16:00 ET. La clasificación se infiere comparando la ejecución con NBBO previo válido. No identifica al comprador, al vendedor ni apertura de posiciones. Solo se avisa de bloques observados durante la búsqueda; no se garantiza cobertura instantánea de todo el watchlist.</p></section>`;
}

function statusMarkup():string {
 if(!isTauri())return '<p>Abre la aplicación Windows para activar la vigilancia.</p>';
 const s=current;
 return `<p role="status">${s?.enabled?'Vigilancia activada':'Vigilancia detenida'} · ${s?.morningWindow?'Ventana 04:00–16:00 ET':'Fuera de la ventana matutina'} · ${s?.total?`${s.scanned}/${s.total} activos en la vuelta actual`:'Sin vuelta iniciada'}${s?.lastSymbol?` · último ${esc(s.lastSymbol)} ${time(s.lastScanMs)} ET`:''}</p>${localError||s?.error?`<p class="block-alert-error">${esc(localError||s?.error||'')}</p>`:''}<p>${s?.unread??0} sin leer · ${s?.history.length??0} avisos guardados en la vista.</p>`;
}

export function blockAlertHistory():string {
 return `<section class="block-alert-history" aria-label="Historial de alarmas de bloques"><h3>Alarmas de bloques guardadas</h3><div data-block-alert-history>${historyMarkup()}</div></section>`;
}
function historyMarkup():string {
 const rows=current?.history??[];
 return rows.length?`<div class="block-alert-history-list">${rows.map(({alert,read})=>`<article class="${read?'read':'unread'}"><b class="${alert.side==='BUY'?'buy':'sell'}">${alert.side==='BUY'?'COMPRA ESTIMADA':'VENTA ESTIMADA'}</b><strong>${esc(alert.symbol)} · ${money(alert.notional)}</strong><span>${time(alert.executionMs)} ET · ${alert.shares.toLocaleString('en-US')} acciones @ ${money(alert.price)} · ${esc(alert.nbbo)}</span>${read?'':`<button type="button" data-block-alert-read="${esc(alert.key)}">Marcar leída</button>`}</article>`).join('')}</div>`:'<p>Sin alarmas verificadas guardadas todavía.</p>';
}

export function globalBlockAlerts():string {
 const unread=current?.history.filter(item=>!item.read).slice(0,3)??[];
 return unread.length?`<div class="block-alert-banner" role="status"><b>ALARMAS DE BLOQUES · ${current?.unread} sin leer</b>${unread.map(({alert})=>`<span>${esc(alert.symbol)} ${alert.side==='BUY'?'COMPRA':'VENTA'} ${money(alert.notional)} · ${time(alert.executionMs)} ET</span>`).join('')}<button type="button" data-block-alert-open>Ver historial</button><button type="button" data-block-alert-read-all>Marcar leídas</button></div>`:'';
}

function draw():void {
 const banner=document.querySelector<HTMLElement>('[data-global-block-alerts]');if(banner)banner.innerHTML=globalBlockAlerts();
 const status=document.querySelector<HTMLElement>('[data-block-alert-status]');if(status)status.innerHTML=statusMarkup();
 const history=document.querySelector<HTMLElement>('[data-block-alert-history]');if(history)history.innerHTML=historyMarkup();
 const stop=document.querySelector<HTMLButtonElement>('[data-block-alert-stop]');if(stop)stop.disabled=!current?.enabled||loading;
 const toggle=document.querySelector<HTMLButtonElement>('[data-block-alert-toggle]');if(toggle)toggle.disabled=loading||!isTauri();
 bindActions();
}

async function poll():Promise<void> {
 if(!isTauri()||loading)return;
 try{current=await invoke<Status>('get_block_alerts');draw();}catch(error){localError=String(error);draw();}
}

export function startBlockAlertWatcher():void {
 if(timer!==undefined)return;
 void poll();
 timer=window.setInterval(()=>{void poll();},7000);
}

function bindActions():void {
 const form=document.querySelector<HTMLFormElement>('[data-block-alert-form]');
 if(form)form.onsubmit=async event=>{
  event.preventDefault();if(loading||!isTauri())return;
  const data=new FormData(form),scope=data.get('scope')==='watchlist'?'watchlist':{selected:parseSymbols(String(data.get('symbols')??''))};
  const config:Config={scope,minimumNotional:Number(data.get('minimum')),alertBuy:data.has('buy'),alertSell:data.has('sell')};
  loading=true;localError='';draw();
  try{current=await invoke<Status>('set_block_alerts',{enabled:true,config,watchlist:scope==='watchlist'?watchlist:[]});}
  catch(error){localError=String(error);}finally{loading=false;draw();}
 };
 const stop=document.querySelector<HTMLButtonElement>('[data-block-alert-stop]');
 if(stop)stop.onclick=async()=>{if(!current?.config||loading)return;loading=true;try{current=await invoke<Status>('set_block_alerts',{enabled:false,config:current.config,watchlist:current.config.scope==='watchlist'?watchlist:[]});localError='';}catch(error){localError=String(error);}finally{loading=false;draw();}};
 document.querySelectorAll<HTMLButtonElement>('[data-block-alert-read]').forEach(button=>button.onclick=()=>{void mark(button.dataset.blockAlertRead);});
 const all=document.querySelector<HTMLButtonElement>('[data-block-alert-read-all]');if(all)all.onclick=()=>{void mark();};
 const open=document.querySelector<HTMLButtonElement>('[data-block-alert-open]');if(open)open.onclick=()=>{document.querySelector<HTMLButtonElement>('[data-view="investment"]')?.click();document.querySelector<HTMLButtonElement>('[data-ir-tab="purchases"]')?.click();document.querySelector('.block-alert-history')?.scrollIntoView({block:'start'});};
}
async function mark(key?:string):Promise<void> {
 if(!isTauri())return;
 try{current=await invoke<Status>('mark_block_alerts_read',{key:key??null});draw();}catch(error){localError=String(error);draw();}
}
export function bindBlockAlerts():void {bindActions();}
