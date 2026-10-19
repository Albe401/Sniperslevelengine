import {alertSounds,getAlertSoundSettings,saveAlertSoundSettings,enableAlertSound,playAlertSound} from './alert-sound';
import {invoke,isTauri} from './snapshot-observation';
import type {ISeriesApi} from 'lightweight-charts';
import {escapeHtml as esc} from './model';

export interface PriceAlert {id:string;price:number;direction:'up'|'down'|'either';sound:boolean;armedAt?:number;firedAt?:number;firedPrice?:number}
export interface AlertSample {price:number;at:number}
export function crossedAlert(a:PriceAlert,previous:AlertSample|undefined,current:AlertSample,now:number):boolean {
 if(a.firedAt!=null||!previous||![a.price,previous.price,current.price,current.at,previous.at,now].every(Number.isFinite)||a.price<=0||previous.price<=0||current.price<=0)return false;
 if(current.at<=previous.at||current.at>now+1000||now-current.at>15000||current.at-previous.at>15000)return false;
 const up=previous.price<a.price&&current.price>=a.price,down=previous.price>a.price&&current.price<=a.price;
 return a.direction==='up'?up:a.direction==='down'?down:up||down;
}
export function validAlerts(v:unknown):PriceAlert[]{
 if(!Array.isArray(v))return [];const ids=new Set<string>();
 return v.slice(0,30).filter((a):a is PriceAlert=>!!a&&typeof a.id==='string'&&a.id.length<=80&&!ids.has(a.id)&&!!ids.add(a.id)&&Number.isFinite(a.price)&&a.price>0&&['up','down','either'].includes(a.direction)&&typeof a.sound==='boolean'&&(a.firedAt==null||Number.isFinite(a.firedAt)&&a.firedAt>0)).map(a=>({...a}));
}
const states=new Map<string,PriceAlert[]>(),previous=new Map<string,AlertSample>();
let error='';
function rows(symbol:string){let r=states.get(symbol);if(!r){try{r=validAlerts(JSON.parse(localStorage.getItem('snipers.alerts.v1.'+symbol)??'[]'));}catch{r=[];}states.set(symbol,r);}return r;}
function save(symbol:string,sync=true){try{localStorage.setItem('snipers.alerts.v1.'+symbol,JSON.stringify(rows(symbol)));error='';}catch{error='No se pudo guardar: las alarmas quedan en memoria.';}if(sync)syncBackendAlerts();}
const direction=(d:PriceAlert['direction'])=>d==='up'?'↑ Subida':d==='down'?'↓ Bajada':'↕ Cruce';
function alertSoundControls(){const current=getAlertSoundSettings();return `<select data-alert-tone aria-label="Sonido de todas las alarmas" title="Se aplica a todas las alarmas con sonido">${Object.entries(alertSounds).map(([id,label])=>`<option value="${id}" ${current.tone===id?'selected':''}>${label}</option>`).join('')}</select><button type="button" data-alert-sound-test>Probar sonido</button>`;}
function bindAlertSound(root:HTMLElement){
 const select=root.querySelector<HTMLSelectElement>('[data-alert-tone]'),test=root.querySelector<HTMLButtonElement>('[data-alert-sound-test]');
 const message=(text:string)=>{const el=root.querySelector('[data-alert-error]');if(el)el.textContent=text;};
 if(select)select.onchange=()=>{const saved=saveAlertSoundSettings({...getAlertSoundSettings(),tone:select.value});message(saved?'Sonido guardado para todas las alarmas.':'Sonido cambiado; no se pudo guardar para el próximo inicio.');void enableAlertSound();};
 if(test)test.onclick=async()=>{const enabled=await enableAlertSound();message(enabled&&playAlertSound()?'Prueba de sonido.':'No se pudo activar el audio. Revisa la salida de sonido.');};
}
function list(symbol:string){return rows(symbol).map(a=>`<div class="chart-alert-row"><b>$${a.price.toFixed(2)}</b><span>${direction(a.direction)} · ${a.firedAt?`Activada ${new Date(a.firedAt).toLocaleTimeString('es',{timeZone:'America/New_York',hour12:false})} ET`:'Armada'}</span>${a.firedAt?`<button type="button" data-alert-rearm="${esc(a.id)}">Rearmar</button>`:''}<button type="button" data-alert-delete="${esc(a.id)}" aria-label="Eliminar alarma de ${a.price}">×</button></div>`).join('')||'<small>Sin alarmas para este activo.</small>';}
export function chartAlertsToolbar(symbol:string){return `<details class="chart-alerts" data-horizon-details="chart-alerts"><summary>Alarmas (${rows(symbol).filter(a=>!a.firedAt).length})</summary><form data-alert-form data-chart-preserve><label>Precio <input name="price" type="number" min="0.0001" step="any" required aria-label="Precio de la alarma"></label><select name="direction" aria-label="Dirección del cruce"><option value="either">Cualquier cruce</option><option value="up">Cruce hacia arriba</option><option value="down">Cruce hacia abajo</option></select><label><input name="sound" type="checkbox" checked> Sonido</label>${alertSoundControls()}<button type="submit">Crear alarma</button></form><div data-alert-list>${list(symbol)}</div><small>Vigila todos los activos con alarmas mientras el motor esté abierto y Data conectado. Un aviso por cruce; sin órdenes automáticas. Abre Alarmas para habilitar el sonido tras reiniciar.</small><span data-alert-error>${esc(error)}</span></details><div class="chart-alert-notice" data-chart-preserve role="status" aria-live="polite" hidden></div>`;}
export class ChartAlerts {
 private symbol='';private lines:ReturnType<ISeriesApi<'Candlestick'>['createPriceLine']>[]=[];private signature='';
 constructor(private series:ISeriesApi<'Candlestick'>){}
 bind(root:HTMLElement,symbol:string,sample:AlertSample|undefined,now:number){
  if(this.symbol!==symbol){previous.delete(this.symbol);previous.delete(symbol);this.symbol=symbol;this.signature='';root.querySelector<HTMLElement>('.chart-alert-notice')?.setAttribute('hidden','');}
  const enableSound=()=>{if(allPriceAlerts().some(a=>a.sound))void enableAlertSound();};
  const summary=root.querySelector<HTMLElement>('.chart-alerts summary');if(summary)summary.onclick=enableSound;
  const form=root.querySelector<HTMLFormElement>('[data-alert-form]');
  if(form)form.onsubmit=e=>{e.preventDefault();const data=new FormData(form),price=Number(data.get('price')),d=data.get('direction');if(!Number.isFinite(price)||price<=0||rows(symbol).length>=30||!['up','down','either'].includes(String(d)))return;
   rows(symbol).push({id:crypto.randomUUID(),price,direction:d as PriceAlert['direction'],sound:data.get('sound')==='on',armedAt:Date.now()});previous.delete(symbol);save(symbol);
   if(data.get('sound')==='on')void enableAlertSound();
   this.update(root);form.reset();const tone=form.querySelector<HTMLSelectElement>("[data-alert-tone]");if(tone)tone.value=getAlertSoundSettings().tone;};
  bindAlertSound(root);this.actions(root);
  if(isTauri()){this.update(root);return;}
  if(!sample||!Number.isFinite(sample.at)||!Number.isFinite(sample.price)||sample.price<=0||sample.at>now+1000||now-sample.at>15000){previous.delete(symbol);}
  else {const prev=previous.get(symbol),hits=rows(symbol).filter(a=>crossedAlert(a,prev,sample,now));
   if(hits.length){for(const a of hits)a.firedAt=now;save(symbol);this.update(root);const notice=root.querySelector<HTMLElement>('.chart-alert-notice');if(notice){notice.hidden=false;notice.textContent=`ALARMA · ${symbol} · ${hits.map(a=>'$'+a.price.toFixed(2)+' '+direction(a.direction)).join(' · ')} · precio $${sample.price.toFixed(2)}`;}
    if(hits.some(a=>a.sound))playAlertSound();
   }
   if(!prev||sample.at>prev.at)previous.set(symbol,{...sample});
  }
  this.paint();
 }
 private actions(root:HTMLElement){root.querySelectorAll<HTMLButtonElement>('[data-alert-delete]').forEach(b=>b.onclick=()=>{states.set(this.symbol,rows(this.symbol).filter(a=>a.id!==b.dataset.alertDelete));save(this.symbol);this.update(root);});root.querySelectorAll<HTMLButtonElement>('[data-alert-rearm]').forEach(b=>b.onclick=()=>{const a=rows(this.symbol).find(a=>a.id===b.dataset.alertRearm);if(a){delete a.firedAt;delete a.firedPrice;a.armedAt=Date.now();previous.delete(this.symbol);save(this.symbol);this.update(root);}});}
 private update(root:HTMLElement){const l=root.querySelector('[data-alert-list]');if(l)l.innerHTML=list(this.symbol);const summary=root.querySelector('.chart-alerts summary');if(summary)summary.textContent=`Alarmas (${rows(this.symbol).filter(a=>!a.firedAt).length})`;const msg=root.querySelector('[data-alert-error]');if(msg)msg.textContent=error;this.actions(root);this.paint();}
 private paint(){const sig=JSON.stringify([this.symbol,rows(this.symbol)]);if(sig===this.signature)return;this.signature=sig;for(const l of this.lines)this.series.removePriceLine(l);this.lines=rows(this.symbol).filter(a=>!a.firedAt).map(a=>this.series.createPriceLine({price:a.price,title:'Alarma '+direction(a.direction),color:'#ffcd57',lineWidth:1,lineStyle:2,axisLabelVisible:true}));}
 dispose(){previous.delete(this.symbol);for(const l of this.lines)this.series.removePriceLine(l);this.lines=[];}
}

export interface BackgroundAlert extends PriceAlert {symbol:string}
export function allPriceAlerts():BackgroundAlert[]{
 try{for(let i=0;i<localStorage.length;i++){const k=localStorage.key(i);if(k?.startsWith('snipers.alerts.v1.')){const symbol=k.slice('snipers.alerts.v1.'.length);if(/^[A-Z][A-Z0-9.-]{0,14}$/.test(symbol))rows(symbol);}}}catch{/* Memory remains usable. */}
 return [...states].flatMap(([symbol,r])=>r.map(a=>({...a,symbol,armedAt:a.armedAt??0,firedAt:a.firedAt??null,firedPrice:a.firedPrice??null}))) as unknown as BackgroundAlert[];
}
let syncQueue=Promise.resolve();
function syncBackendAlerts(){if(!isTauri())return;syncQueue=syncQueue.catch(()=>{}).then(async()=>{try{await invoke('set_price_alerts',{alerts:allPriceAlerts()});error='';}catch{error='Guardadas; vigilancia pendiente de conexión Data.';}});}
let openAsset:(symbol:string)=>void=()=>{};
export function configureBackgroundAlerts(open:(symbol:string)=>void){openAsset=open;allPriceAlerts();document.addEventListener('pointerdown',()=>{if(allPriceAlerts().some(a=>a.sound))void enableAlertSound();});}
export function consumeBackgroundAlerts(alerts:BackgroundAlert[]){
 for(const a of alerts){const own=rows(a.symbol).find(r=>r.id===a.id&&(r.armedAt??0)===(a.armedAt??0));if(!own||a.firedAt==null||own.firedAt!=null)continue;
  own.firedAt=a.firedAt;own.firedPrice=a.firedPrice;save(a.symbol,false);
  let container=document.getElementById('background-price-alerts');if(!container){container=document.createElement('div');container.id='background-price-alerts';container.setAttribute('role','status');container.setAttribute('aria-live','polite');document.body.append(container);}
  const notice=document.createElement('div');notice.className='background-price-alert';const text=document.createElement('span');text.textContent=`${a.symbol} · ALARMA $${a.price.toFixed(2)} · ${direction(a.direction)} · ${new Date(a.firedAt).toLocaleTimeString('es',{timeZone:'America/New_York',hour12:false})} ET`;
  const open=document.createElement('button');open.textContent='Abrir '+a.symbol;open.onclick=()=>openAsset(a.symbol);const close=document.createElement('button');close.textContent='×';close.setAttribute('aria-label','Cerrar aviso');close.onclick=()=>notice.remove();notice.append(text,open,close);container.prepend(notice);while(container.children.length>10)container.lastElementChild?.remove();
  if(a.sound)playAlertSound();
 }
}
/** Price-axis shortcut; picking a level never submits or draws an order. */
export class ChartAlarmPicker {
 private button:HTMLButtonElement;private guide:HTMLDivElement;private label:HTMLSpanElement;private price:number|undefined;private symbol='';
 constructor(private host:HTMLElement,private series:ISeriesApi<'Candlestick'>){
  this.button=document.createElement('button');this.button.type='button';this.button.className='chart-alarm-plus';this.button.hidden=true;this.button.textContent='+';
  this.guide=document.createElement('div');this.guide.className='chart-alarm-guide';this.guide.hidden=true;this.label=document.createElement('span');this.guide.append(this.label);host.append(this.guide,this.button);
  host.addEventListener('pointermove',this.move);host.addEventListener('pointerleave',this.leave);this.button.onpointerdown=e=>e.stopPropagation();
  this.button.onclick=e=>{e.preventDefault();e.stopPropagation();if(this.price==null)return;const root=host.closest<HTMLElement>('.gamma-live'),details=root?.querySelector<HTMLDetailsElement>('.chart-alerts'),input=root?.querySelector<HTMLInputElement>('[data-alert-form] input[name="price"]');if(details&&input){details.open=true;input.value=this.price.toFixed(2);input.focus({preventScroll:true});}};
 }
 bind(symbol:string){if(this.symbol!==symbol){this.symbol=symbol;this.price=undefined;this.hide();}}
 private hide(){this.button.hidden=true;this.guide.hidden=true;}
 private move=(e:PointerEvent)=>{if(e.target===this.button)return;const y=e.clientY-this.host.getBoundingClientRect().top,p=this.series.coordinateToPrice(y);if(y<12||y>(Number(this.host.dataset.pricePaneHeight)||this.host.clientHeight)-12||p==null||!Number.isFinite(p)||p<=0){this.hide();return;}const axis=Math.max(1,this.series.priceScale().width());this.price=Number(p);this.button.hidden=false;this.guide.hidden=false;this.button.style.top=Math.max(0,y-12)+'px';this.button.style.right=(axis+4)+'px';this.guide.style.top=y+'px';this.guide.style.right=axis+'px';this.label.textContent=Number(p).toFixed(2);this.button.title='Crear alarma en $'+Number(p).toFixed(2);this.button.setAttribute('aria-label','Crear alarma en '+Number(p).toFixed(2));};
 private leave=()=>this.hide();
 dispose(){this.host.removeEventListener('pointermove',this.move);this.host.removeEventListener('pointerleave',this.leave);this.button.remove();this.guide.remove();}
}
