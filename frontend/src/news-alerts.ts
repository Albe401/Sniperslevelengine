import {queryNews,newsEventKey,readyNews,unseenRevisions} from './news-pipeline';
import {invoke,isTauri} from './snapshot-observation';
import {enableAlertSound,playAlertSound} from './alert-sound';
import {escapeHtml} from './model';
import {publishMarketNews,newsSymbols,type MarketNewsReport} from './market-news';
import {watchedNewsSymbols} from './news-watchlist';
import {newsCard,type RadarNews} from './investment-news';
import './news-alerts.css';

const key='snipers.news-alarm.v1';
let enabled=true,onlyImportant=false,watchOnly=true;
try {const saved=JSON.parse(localStorage.getItem(key)??'null');if(saved){enabled=saved.enabled!==false;onlyImportant=saved.onlyImportant===true;watchOnly=saved.watchOnly!==false;}}catch{/* Defaults. */}
let seen=new Set<string>(),baseline=false,busy=false,started=false,recheck=false;
function persistAlerts(items:RadarNews[]){const known=items.filter(n=>n.revision!=null).slice(0,64);if(known.length)void invoke('record_news_alerts',{items:known.map(n=>({id:n.id,revision:n.revision,enriched:!!n.translated}))}).catch(()=>{status='Aviso visible · registro de evidencia pendiente';refresh();});}
let initialized=new Set<string>();
const eventSeen=new Set<string>();
let latest:RadarNews[]=[],popup:RadarNews[]=[];
let status='Esperando noticias nuevas';
let menuOpen=false;
export function newNews(items:RadarNews[],seenIds:Set<string>):RadarNews[]{
 const unique=new Map<string,RadarNews>();
 for(const item of items)if(item.id&&!seenIds.has(item.id))unique.set(item.id,item);
 return [...unique.values()];
}
export function importantNews(item:RadarNews):boolean{return item.rawCritical===true||item.critical===true||item.importance==='high'||item.eventType==='upgrade'||item.eventType==='downgrade';}
export function watchedNews(item:RadarNews,symbols=watchedNewsSymbols()):boolean {
 return newsSymbols(item).some(s=>symbols.includes(s));
}
export function audibleNews(items:RadarNews[],symbols:string[],scopeOnly:boolean,importantOnly:boolean):RadarNews[] {
 return items.filter(n=>watchedNews(n,symbols)||!scopeOnly&&(!importantOnly||importantNews(n)));
}
export function globalNewsAlerts():string{
 const symbols=watchedNewsSymbols(),visiblePopup=popup.filter(n=>watchedNews(n,symbols));
 return `<div class="global-news-alarm"><button class="news-alarm-compact" data-news-alarm-menu aria-expanded="${menuOpen}" aria-controls="news-alarm-menu" title="${escapeHtml(status)} · Vigilando: ${escapeHtml(symbols.join(' · '))}"><span aria-hidden="true">🔔</span> Noticias${visiblePopup.length?` <b>${visiblePopup.length}</b>`:''}<span class="news-alarm-state ${enabled?'is-enabled':''}" aria-label="${enabled?'Campana activada':'Campana silenciada'}"></span></button><aside id="news-alarm-menu" class="news-alarm-menu" ${menuOpen?'':'hidden'}><strong>Noticias · alarma global</strong> <button data-news-alarm-toggle>${enabled?'Sonido activado':'Sonido desactivado'}</button> <label><input type="checkbox" data-news-alarm-watch ${watchOnly?'checked':''}> Solo mis cinco activos</label> ${!watchOnly?`<label><input type="checkbox" data-news-alarm-important ${onlyImportant?'checked':''}> Solo importantes del resto del mercado</label>`:''} <button data-news-alarm-test>Probar campana</button><span class="news-watch-symbols">Vigilando: ${escapeHtml(symbols.join(' · '))}</span><span>${escapeHtml(status)}</span>${latest.length?`<details><summary>${latest.length} noticias nuevas · ver titulares</summary>${latest.slice(0,20).map(n=>`<p>${escapeHtml(n.ticker)} · ${escapeHtml(n.title)}</p>`).join('')}</details>`:''}</aside></div>`;
}
export function globalNewsPopup():string{
 const visiblePopup=popup.filter(n=>watchedNews(n));
 return visiblePopup.length?`<aside class="news-watch-popup ir-news-channel" aria-label="Noticias nuevas de tus cinco activos"><header><strong role="status">Noticia nueva de tus activos · ${visiblePopup.length}</strong><button data-news-popup-dismiss>Cerrar avisos</button></header><div class="news-watch-popup-list">${visiblePopup.map(newsCard).join('')}</div><small>Consulta cada 60 s · aviso original para críticos reportados · traducción posterior</small></aside>`:'';
}
function refresh(){const host=document.querySelector<HTMLElement>('[data-global-news-alerts]');if(host)host.innerHTML=globalNewsAlerts();const popupHost=document.querySelector<HTMLElement>('[data-news-popup-host]');if(popupHost)popupHost.innerHTML=globalNewsPopup();if(host||popupHost)bindNewsAlerts();}
function save(){try{localStorage.setItem(key,JSON.stringify({enabled,onlyImportant,watchOnly}));}catch{/* Session settings still apply. */}refresh();}
export function bindNewsAlerts():void{
 const menu=document.querySelector<HTMLButtonElement>('[data-news-alarm-menu]');if(menu)menu.onclick=()=>{menuOpen=!menuOpen;refresh();};
 const toggle=document.querySelector<HTMLButtonElement>('[data-news-alarm-toggle]');if(toggle)toggle.onclick=()=>{enabled=!enabled;if(enabled)void enableAlertSound();save();};
 const scope=document.querySelector<HTMLInputElement>('[data-news-alarm-watch]');if(scope)scope.onchange=()=>{watchOnly=scope.checked;save();};
 const important=document.querySelector<HTMLInputElement>('[data-news-alarm-important]');if(important)important.onchange=()=>{onlyImportant=important.checked;save();};
 const dismiss=document.querySelector<HTMLButtonElement>('[data-news-popup-dismiss]');if(dismiss)dismiss.onclick=()=>{popup=[];refresh();};
 const test=document.querySelector<HTMLButtonElement>('[data-news-alarm-test]');if(test)test.onclick=async()=>{status=await enableAlertSound()&&playAlertSound('bell')?'Prueba de campana enviada':'No se pudo activar el sonido';refresh();};
}
export async function pollNewsAlerts():Promise<void>{
 if(busy){recheck=true;return;}if(!isTauri())return;
 busy=true;
 const symbols=watchedNewsSymbols(),previous=new Set(initialized);
 try{
  const next=await queryNews(symbols),items=next.items;
  const failures=next.coverage.failures,partial=next.report?.truncated??true;
  for(const scope of next.coverage.symbols)if(scope.status!=='error'&&watchedNewsSymbols().includes(scope.symbol))initialized.add(scope.symbol);
  if(next.report)publishMarketNews(next.report);
  if(!next.report){status='No se pudieron actualizar las noticias · se reintentará';return;}  const current=watchedNewsSymbols();
  const fresh=unseenRevisions(items,seen,eventSeen).filter(n=>readyNews(n)&&n.horizon==='intraday'&&Number.isFinite(Date.parse(n.publishedAt))&&Date.parse(n.publishedAt)<=Date.now()&&Date.now()-Date.parse(n.publishedAt)<=86400000);
  for(const item of items)if(readyNews(item)||!baseline)seen.add(item.id);
  const enrichedUpdates=popup.filter(old=>!old.translated).flatMap(old=>items.filter(n=>n.id===old.id&&(n.revision??1)===(old.revision??1)&&n.translated));persistAlerts(enrichedUpdates);
  popup=popup.map(old=>items.find(n=>n.id===old.id&&(n.revision??1)===(old.revision??1)&&n.translated)??old);
  if(seen.size>10000)seen=new Set([...seen].slice(-8000));
  if(baseline){
   const eligible=fresh.filter(n=>!watchedNews(n,current)||newsSymbols(n).some(s=>current.includes(s)&&previous.has(s)));
   const audible=audibleNews(eligible,current,watchOnly,onlyImportant).filter(n=>{const key=newsEventKey(n);if(eventSeen.has(key))return false;eventSeen.add(key);return true;});
   for(const n of audible)eventSeen.add(newsEventKey(n));if(eventSeen.size>10000){const keep=[...eventSeen].slice(-8000);eventSeen.clear();keep.forEach(k=>eventSeen.add(k));}
   const watched=audible.filter(n=>watchedNews(n,current));
   if(audible.length){
    persistAlerts(audible);
    latest=[...new Map([...latest,...audible].map(n=>[n.id,n])).values()].sort((a,b)=>Date.parse(b.publishedAt)-Date.parse(a.publishedAt)).slice(0,50);
    popup=[...watched,...popup.filter(old=>!watched.some(n=>n.id===old.id||newsEventKey(n)===newsEventKey(old)))].filter(n=>watchedNews(n,current)).slice(0,5);
    status=`${audible.length} noticias nuevas · ${watched.length} de tus cinco activos`;
    if(enabled&&!playAlertSound('bell'))status+=' · pulsa Probar campana para activarlo';
   }else status='Vigilancia activa · sin noticias nuevas';
  }else status='Alarma lista · carga inicial sin sonido';
  if(failures)status+=` · ${failures} consultas pendientes`;
  if(partial)status+=' · cobertura parcial';
  if(!baseline)items.forEach(n=>eventSeen.add(newsEventKey(n)));
  baseline=true;
 }finally{busy=false;refresh();if(recheck){recheck=false;void pollNewsAlerts();}}
}
export function startNewsAlertWatcher():void{
 if(started)return;started=true;
 document.addEventListener('pointerdown',()=>{if(enabled)void enableAlertSound();},{passive:true});
 document.addEventListener('pointerdown',event=>{if(menuOpen&&event.target instanceof Element&&!event.target.closest('[data-global-news-alerts]')){menuOpen=false;refresh();}});
 document.addEventListener('keydown',event=>{if(event.key==='Escape'&&menuOpen){menuOpen=false;refresh();document.querySelector<HTMLButtonElement>('[data-news-alarm-menu]')?.focus();}});
 document.addEventListener('snipers:news-watchlist',()=>{
  const current=watchedNewsSymbols();initialized=new Set([...initialized].filter(s=>current.includes(s)));
  popup=popup.filter(n=>watchedNews(n,current));refresh();void pollNewsAlerts();
 });
 void pollNewsAlerts();setInterval(()=>void pollNewsAlerts(),60000);
}




