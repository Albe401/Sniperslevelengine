import {rankNewsAssets,assetNewsPanel,topFiveNewsMethod} from './top-five-news';
import {nativeOptionsPanel,refreshNativeOptions} from './native-top-five';
import {botUniversePanel,bindBotUniverse} from './bot-universe';
import {topFiveVolumeMetrics,type TopFiveVolumeMetrics} from './top-five-volume-metrics';
import {sectorActivityPanel} from './sector-activity';
import {manualScannerPanel,bindManualScanner} from './manual-scanner';
import {escapeHtml as esc,price} from './model';
import {interpretationFresh} from './confirmations';
import type {Scanner} from './scanner';
export interface VolumeWindow {symbol:string;period:string;startMs:number;endMs:number;finalized:boolean;volume:number;price:number;changePct:number;bars:number;volumeMetrics?:TopFiveVolumeMetrics}
const date=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York',year:'numeric',month:'2-digit',day:'2-digit'});
const clock=new Intl.DateTimeFormat('es',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit',hourCycle:'h23'});
const number=new Intl.NumberFormat('es',{maximumFractionDigits:0});
export function rankVolume(rows:VolumeWindow[],period:string,now:number,limit=5):VolumeWindow[]{
 const candidates=rows.filter(r=>r.period===period&&r.startMs>0&&r.endMs>r.startMs&&r.endMs<=now&&(!period.startsWith('volume')||now-r.endMs<=120000)&&date.format(r.startMs)===date.format(now)&&Number.isFinite(r.volume)&&r.volume>0&&Number.isFinite(r.price)&&r.price>0&&Number.isFinite(r.changePct));
 return candidates.filter(r=>candidates.filter(x=>x.symbol===r.symbol).length===1).sort((a,b)=>b.volume-a.volume||a.symbol.localeCompare(b.symbol)).slice(0,limit);
}
export type ScannerPeriod='premarket'|'opening'|'volume10'|'volume30';
export interface ScannerView {period:ScannerPeriod;sector:string}
export const scannerView:ScannerView={period:'volume10',sector:''};
let nativePool:VolumeWindow[]=[],nativePaint:()=>void=()=>{};
if(typeof document!=='undefined')if(typeof document!=='undefined')document.addEventListener('snipers:market-news',()=>nativePaint());
setInterval(()=>{void refreshNativeOptions(nativePool,nativePaint);},60000);
export function bindTopFive(refresh:()=>void){
 nativePaint=refresh;void refreshNativeOptions(nativePool,refresh);
 bindManualScanner();bindBotUniverse();
 document.querySelectorAll<HTMLButtonElement>('[data-scanner-period]').forEach(b=>b.onclick=()=>{scannerView.period=b.dataset.scannerPeriod as ScannerPeriod;refresh();});
 const select=document.querySelector<HTMLSelectElement>('[data-scanner-sector]');
 if(select)select.onchange=()=>{scannerView.sector=select.value;refresh();};
}
export function topFivePanel(data:Scanner|undefined,active:boolean,published:number|undefined,now=Date.now(),view:ScannerView=scannerView):string{
 const fresh=!!(active&&data?.connected&&interpretationFresh(published,now));
 const recent=(end:number|null|undefined)=>fresh&&end!=null&&end<=now&&now-end<=120000&&date.format(end)===date.format(now);
 const sectors=[...new Set((data?.rows??[]).map(r=>r.sector??'Sin sector'))].sort();
 const inSector=(symbol:string)=>!view.sector||data?.rows.find(r=>r.symbol===symbol)?.sector===view.sector||(view.sector==='Sin sector'&&data?.rows.some(r=>r.symbol===symbol&&!r.sector));
 const windows=fresh?(data?.volumeWindows??[]).filter(r=>inSector(r.symbol)):[];
 nativePool=rankVolume(windows,view.period,now,20);
 const ranked=rankNewsAssets(nativePool,now);
 const rolling=view.period.startsWith('volume');
 const periodLabel=view.period==='volume30'?'últimos 30 minutos':view.period==='volume10'?'últimos 10 minutos':view.period==='opening'?'primeros 15 minutos':'premarket';
 const val=(n:number|null|undefined)=>n==null||!Number.isFinite(n)?'—':price(n);
 const cards=Array.from({length:5},(_,i)=>{
  const r=ranked[i];
  if(!r)return `<article class="scanner-card scanner-empty"><span class="scanner-position">0${i+1}</span><h3>Esperando activo</h3><div class="scanner-volume">—</div><p>Sin clasificación disponible</p><div class="scanner-placeholder"></div><small>Se mostrará al recibir volumen verificado del periodo.</small></article>`;
  const current=data?.rows.find(x=>x.symbol===r.symbol);
  const up=r.changePct>=0;
  return `<article class="scanner-card"><div class="scanner-card-head"><span class="scanner-position">0${i+1}</span><span class="scanner-change ${up?'is-up':'is-down'}">${up?'+':''}${r.changePct.toFixed(2)}%</span></div><h3>${esc(r.symbol)}</h3><small>${esc(current?.sector??'Sin sector')} · cambio del periodo</small><div class="scanner-volume">${number.format(r.volume)}<small>acciones negociadas</small></div><div class="top-five-meter"><span style="width:${r.volume/Math.max(...ranked.map(x=>x.volume))*100}%"></span></div><div class="scanner-card-metrics"><div><small>Cierre del periodo</small><b>${val(r.price)}</b></div></div>${topFiveVolumeMetrics(r.volumeMetrics)}${nativeOptionsPanel(r,now)}${assetNewsPanel(r,nativePool,now)}<small class="scanner-cutoff">${rolling?'Último corte':r.finalized?'Corte final':'Acumulando'} ${clock.format(r.endMs)} ET · ${r.bars} velas</small></article>`;
 }).join('');
 const summary=(period:string,title:string)=>{const rows=rankVolume(windows,period,now);return `<section class="scanner-session"><h3>${title}</h3>${rows.map((r,i)=>`<div><span>${i+1}. <b>${esc(r.symbol)}</b></span><strong>${number.format(r.volume)} <small>acciones</small></strong></div>`).join('')||'<p>Esperando volumen del periodo</p>'}</section>`;};
 const rows=(data?.rows??[]).filter(r=>inSector(r.symbol));
 return `<header class="scanner-heading"><div><span class="eyebrow">EXPLORACIÓN MULTIACTIVO</span><h1>Top 5 <span>· Market scanner</span></h1><p>Encuentra dónde se concentra el volumen de hoy.</p></div><span class="scanner-feed ${fresh?'is-up':''}">${fresh?'Recepción activa':'Sin conexión vigente'}</span></header>
 <p class="panel-caption">Opciones nativas Snipers; hasta 20 candidatos por volumen. Prioridad descriptiva: 40% volumen/OI, 25% concentración GEX a ±2%, 20% RVOL y 15% movimiento observado. Cobertura gamma por OI mínima 90%; cobertura parcial penaliza el índice y se marca provisional. No es probabilidad ni señal de entrada; datos pendientes al final.</p><div data-bot-universe>${botUniversePanel(data?.botSelection)}</div>
 ${manualScannerPanel()}
 <div class="scanner-stats"><div><small>Universo configurado</small><strong>${data?.total??0}</strong><span>acciones y ETF</span></div><div><small>Históricos recuperados</small><strong>${data?.historicalReady??0}<em> / ${data?.total??0}</em></strong><span>cobertura del universo</span></div><div><small>Lecturas 2m vigentes</small><strong>${(data?.rows??[]).filter(r=>recent(r.barEndMs)).length}</strong><span>sesión regular</span></div><div><small>Candidatos por volumen</small><strong>${ranked.length}<em> / 5</em></strong><span>${fresh&&data?.total&&data.historicalReady===data.total?'Universo recuperado':'Clasificación provisional'}</span></div></div>
 <div class="scanner-toolbar"><div class="scanner-periods" role="group" aria-label="Periodo de volumen">${[['volume10','Volumen 10m','Automático'],['volume30','Volumen 30m','Automático'],['premarket','Premarket','04:00–09:30'],['opening','Apertura','09:30–09:45']].map(([id,title,time])=>`<button data-scanner-period="${id}" aria-pressed="${view.period===id}" class="${view.period===id?'selected':''}">${title}<small>${time} ET</small></button>`).join('')}</div><label>Sector <select data-scanner-sector aria-label="Filtrar scanner por sector"><option value="">Todos los sectores</option>${sectors.map(s=>`<option value="${esc(s)}" ${view.sector===s?'selected':''}>${esc(s)}</option>`).join('')}</select></label></div>
 <div class="scanner-section-title"><h2>Cinco activos para observar</h2><span>Opciones + noticias · ${periodLabel}</span></div><div class="scanner-cards">${cards}</div>
 <p class="scanner-note">Preselección por volumen de acciones; orden final por actividad y concentración de opciones verificadas. ${topFiveNewsMethod} El análisis para invertir está en Lectura rápida y Lectura del activo.</p>
 ${sectorActivityPanel(data,active,published,now)}
 <div class="scanner-comparison">${summary('premarket','Premarket · 04:00–09:30 ET')}${summary('opening','Apertura · 09:30–09:45 ET')}</div>
 <section class="panel scanner-universe"><div class="scanner-section-title"><h2>Universo y calidad de datos</h2><span>${rows.length} activos · ${view.sector?esc(view.sector):'todos los sectores'}</span></div><div class="table-wrap"><table><thead><tr><th>ACTIVO</th><th>SECTOR</th><th>CIERRE 2M</th><th>VOLUMEN 2M</th><th>ESTADO DEL DATO</th></tr></thead><tbody>${rows.map(r=>`<tr><td><b>${esc(r.symbol)}</b></td><td>${esc(r.sector??'—')}</td><td>${recent(r.barEndMs)?val(r.price):'—'}</td><td>${recent(r.barEndMs)&&r.volume2m!=null?number.format(r.volume2m):'—'}</td><td>${fresh?esc(r.status):'Sin conexión vigente'}</td></tr>`).join('')||'<tr><td colspan="5">Conecta Massive en Mercado en vivo para cargar el universo.</td></tr>'}</tbody></table></div></section><p class="scanner-note">Volumen de acciones de agregados de minuto, dentro del universo configurado; no representa todo el mercado. Cortes de hoy conservados al finalizar cada periodo. Datos recuperados y ventanas ausentes permanecen identificados; no se generan valores de ejemplo.</p>`;
}
