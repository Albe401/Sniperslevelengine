import type {LiveState} from './live';
import {escapeHtml as esc} from './model';

export interface FlowCell {ticker:string;expiration:string;kind:string;strike:number;startMs:number;endMs:number;category:string;contracts:number;executions:number;verifiedPremium:number;premium:number|null;missingPremiumExecutions:number}
export interface FlowMapReport {storageError?:string|null;method:string;symbol:string;date:string|null;asofMs:number;connected:boolean;firstReceivedMs:number|null;lastReceivedMs:number|null;population:string;status:string;receptionCoveragePct:number|null;cells:FlowCell[];diagnostics:Record<string,number>}
export interface MapSelection {symbol:string;view:'levels'|'flow';metric:'contracts'|'premium';expiry:string;category:string;strike:number|null;minute:number|null;side?:'call'|'put'}
export const newSelection=(symbol:string):MapSelection=>({symbol,view:'levels',metric:'contracts',expiry:'',category:'all',strike:null,minute:null});
let selection=newSelection('');
const categories=['single','auction_cross_floor','late','complex','extended'];
const categoryNames:Record<string,string>={all:'Todas · separadas en detalle',single:'Simples',auction_cross_floor:'Subastas / cruces / piso',late:'Tardías',complex:'Multileg',extended:'Fuera de horario'};
const money=(n:number|null)=>n==null?'—':new Intl.NumberFormat('es-ES',{style:'currency',currency:'USD',maximumFractionDigits:2}).format(n);
const number=(n:number)=>new Intl.NumberFormat('es-ES',{maximumFractionDigits:0}).format(n);
const price=(n:number)=>n.toFixed(2).replace('.',',');
const clock=(t:number)=>new Intl.DateTimeFormat('es-ES',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit',hour12:false}).format(t);
const dateET=(t:number)=>new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'}).format(t);
const finite=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n);
const count=(n:unknown):n is number=>finite(n)&&Number.isSafeInteger(n)&&n>=0;

export function usableReport(r:FlowMapReport|undefined,symbol:string,now:number):r is FlowMapReport {
 return !!r&&r.method==='option-execution-heatmap-v1'&&r.symbol===symbol&&finite(r.asofMs)&&r.asofMs>0&&r.asofMs<=now&&Array.isArray(r.cells)&&r.cells.length<=250000;
}
export function validCell(c:FlowCell,asof:number):boolean {
 return typeof c.ticker==='string'&&c.ticker.length<100&&/^\d{4}-\d{2}-\d{2}$/.test(c.expiration)&&['call','put'].includes(c.kind)&&categories.includes(c.category)&&finite(c.strike)&&c.strike>0&&Number.isSafeInteger(c.startMs)&&c.startMs>=0&&c.startMs%60000===0&&c.endMs===c.startMs+60000&&c.startMs<=asof&&dateET(c.startMs)===dateET(asof)&&count(c.contracts)&&count(c.executions)&&count(c.missingPremiumExecutions)&&c.missingPremiumExecutions<=c.executions&&finite(c.verifiedPremium)&&c.verifiedPremium>=0&&(c.premium===null||finite(c.premium)&&c.premium>=0)&&((c.premium===null)===(c.missingPremiumExecutions>0))&&(c.premium===null||c.premium===c.verifiedPremium);
}
export interface Group {strike:number;kind:string;startMs:number;contracts:number;executions:number;premium:number|null;verifiedPremium:number;tickers:string[];categories:string[]}
/** Same-contract categories add; distinct contracts at a strike are deliberately not merged. */
export function mapGroups(r:FlowMapReport,s:MapSelection):{groups:Group[];rejected:number;expiries:string[]} {
 const valid=r.cells.filter(c=>validCell(c,r.asofMs));
 const expiries=[...new Set(valid.map(c=>c.expiration))].sort();
 const expiry=expiries.includes(s.expiry)?s.expiry:expiries[0];
 const start=Math.floor(r.asofMs/60000)*60000-29*60000;
 const chosen=valid.filter(c=>c.expiration===expiry&&c.startMs>=start&&(s.category==='all'||s.category===c.category));
 const grouped=new Map<string,Group>();let rejected=r.cells.length-valid.length;
 const seen=new Set<string>();
 for(const c of chosen){
  const identity=[c.ticker,c.startMs,c.category].join('|');if(seen.has(identity)){rejected++;continue;}seen.add(identity);
  const key=[c.kind,c.strike,c.startMs].join('|');
  const old=grouped.get(key);
  if(old){old.contracts+=c.contracts;old.executions+=c.executions;old.verifiedPremium+=c.verifiedPremium;old.premium=old.premium==null||c.premium==null?null:old.premium+c.premium;old.tickers.push(c.ticker);old.categories.push(c.category);}
  else grouped.set(key,{strike:c.strike,kind:c.kind,startMs:c.startMs,contracts:c.contracts,executions:c.executions,premium:c.premium,verifiedPremium:c.verifiedPremium,tickers:[c.ticker],categories:[c.category]});
 }
 const groups=[...grouped.values()].filter(g=>{
  const good=new Set(g.tickers).size===1&&count(g.contracts)&&count(g.executions)&&finite(g.verifiedPremium)&&(g.premium==null||finite(g.premium));if(!good)rejected++;return good;
 });
 return {groups,rejected,expiries};
}
const color=(value:number,max:number)=>{const f=max>0?Math.min(1,value/max):0;return f>.66?`rgb(${Math.round(208+47*f)},${Math.round(156+80*f)},${Math.round(60+88*f)})`:`rgb(${Math.round(12+35*f)},${Math.round(30+160*f)},${Math.round(48+210*f)})`;};
function heatChart(groups:Group[],side:string,metric:MapSelection['metric'],strikes:number[],asof:number,max:number,s:MapSelection):string {
 const width=550,left=58,right=15,top=20,rowH=30,height=top+Math.max(1,strikes.length)*rowH+52,plotW=width-left-right,cw=plotW/30;
 const start=Math.floor(asof/60000)*60000-29*60000;
 const lookup=new Map(groups.filter(g=>g.kind===side).map(g=>[[g.strike,g.startMs].join('|'),g]));
 const rows=strikes.map((strike,i)=>`<text x="48" y="${top+i*rowH+20}" text-anchor="end">${price(strike)}</text>`+Array.from({length:30},(_,j)=>{
  const t=start+j*60000,g=lookup.get([strike,t].join('|')),v=g?.[metric];
  const selected=s.strike===strike&&(s.minute==null||s.minute===t)&&(!s.side||s.side===side);
  return `<g tabindex="0" role="button" aria-label="${side.toUpperCase()} ${price(strike)} ${clock(t)}: ${v==null?'sin dato':number(v)}" data-map-strike="${strike}" data-map-minute="${t}" data-map-side="${side}"><title>${side.toUpperCase()} ${price(strike)} · ${clock(t)} · ${v==null?'Sin dato verificable':metric==='premium'?money(v):number(v)+' contratos'}</title><rect x="${left+j*cw}" y="${top+i*rowH}" width="${cw-1}" height="${rowH-1}" fill="${v==null?'url(#snipers-map-missing-'+side+')':color(v,max)}" stroke="${selected?'#f8fafc':'none'}"/><text x="${left+(j+.5)*cw}" y="${top+i*rowH+20}" text-anchor="middle" fill="#708898">${v==null?'·':''}</text></g>`;
 }).join('')).join('');
 const labels=[0,5,10,15,20,25,29].map(j=>`<text x="${left+(j+.5)*cw}" y="${height-20}" text-anchor="middle">${clock(start+j*60000)}</text>`).join('');
 return `<div class="tm-heat"><h3>${side==='call'?'CALLS':'PUTS'}</h3><svg viewBox="0 0 ${width} ${height}" role="img" aria-label="Heatmap ${side.toUpperCase()} por strike y minuto"><defs><pattern id="snipers-map-missing-${side}" width="6" height="6" patternUnits="userSpaceOnUse"><rect width="6" height="6" fill="#0d1922"/><path d="M0 6L6 0" stroke="#1a2b37"/></pattern></defs>${rows}${labels}</svg></div>`;
}
function inspector(groups:Group[],s:MapSelection):string {
 const matches=groups.filter(g=>g.strike===s.strike&&(s.minute==null||g.startMs===s.minute)&&(!s.side||g.kind===s.side));
 const chosen=s.minute==null?['call','put'].flatMap(side=>matches.filter(g=>g.kind===side).sort((a,b)=>a.startMs-b.startMs).slice(-1)):matches;
 if(!chosen.length)return '<aside class="tm-inspector"><h3>DETALLE DEL STRIKE</h3><p>Selecciona una celda para inspeccionar sus operaciones.</p></aside>';
 return `<aside class="tm-inspector"><h3>STRIKE ${price(s.strike!)}</h3>${chosen.slice(-4).map(g=>`<section><h4>${g.kind.toUpperCase()} · ${clock(g.startMs)} ET</h4><dl><dt>Contratos observados</dt><dd>${number(g.contracts)}</dd><dt>Prima bruta</dt><dd>${money(g.premium)}</dd><dt>Ejecuciones</dt><dd>${number(g.executions)}</dd></dl><small>${g.categories.map(c=>esc(categoryNames[c])).join(' · ')}</small></section>`).join('')}<p>Lado agresor sin clasificar en este mapa.</p><button data-map-view="levels">Ver nivel ${price(s.strike!)} →</button></aside>`;
}
function levelChart(state:LiveState|undefined,asof:number,levels:{price:number;name:string}[]):string {
 const source=new Map((state?.chartHistory?.minutes??[]).map(b=>[b.timestampMs,b]));
 for(const b of Object.values(state?.minutes??{}))source.set(b.timestampMs,b);
 const rows=[...source.values()].filter(b=>finite(b.timestampMs)&&b.timestampMs<=asof&&b.timestampMs>=asof-3600000&&dateET(b.timestampMs)===dateET(asof)&&finite(b.close)&&b.close>0).sort((a,b)=>a.timestampMs-b.timestampMs);
 const values=[...rows.map(b=>b.close),...levels.map(l=>l.price)];
 if(!values.length)return '<div class="tm-chart-empty">Esperando historial real de precios y niveles.</div>';
 const low=Math.min(...values),high=Math.max(...values),pad=Math.max((high-low)*.12,.01),min=low-pad,max=high+pad;
 const y=(p:number)=>430-(p-min)/(max-min)*390,x=(t:number)=>65+(t-(asof-3600000))/3600000*760;
 const path=rows.map((b,i)=>`${i===0||b.timestampMs-rows[i-1].timestampMs>60000?'M':'L'}${x(b.timestampMs).toFixed(2)},${y(b.close).toFixed(2)}`).join(' ');
 return `<svg viewBox="0 0 950 480" class="tm-level-chart" role="img" aria-label="Cierres de un minuto y niveles modelados">${Array.from({length:5},(_,i)=>{const p=min+(max-min)*i/4;return `<path d="M65 ${y(p)}H825" class="tm-grid"/><text x="56" y="${y(p)+4}" text-anchor="end">${price(p)}</text>`;}).join('')}${levels.map(l=>`<path d="M65 ${y(l.price)}H825" class="tm-level"/><text x="830" y="${y(l.price)+4}" class="tm-model">${esc(l.name)}</text>`).join('')}<path d="${path}" class="tm-price"/>${[0,15,30,45,60].map(m=>`<text x="${65+m/60*760}" y="465" text-anchor="middle">${clock(asof-3600000+m*60000)}</text>`).join('')}</svg><p class="tm-note">Cierres de 1 minuto · los huecos se conservan · líneas de opciones: modelo, no órdenes.</p>`;
}
export function twoMapsPanel(state:LiveState|undefined,symbol:string,active:boolean,now=Date.now(),input?:MapSelection):string {
 if(!input&&selection.symbol!==symbol)selection=newSelection(symbol);
 const s=input??selection,r=state?.symbol===symbol?state.flow?.heatmap:undefined;
 const usable=usableReport(r,symbol,now),asof=usable?r.asofMs:state?.symbol===symbol&&finite(state.publishedMs)&&state.publishedMs>0&&state.publishedMs<=now?state.publishedMs:now;
 const data=usable?mapGroups(r,s):{groups:[],rejected:0,expiries:[]};
 const expiry=data.expiries.includes(s.expiry)?s.expiry:data.expiries[0]??'';
 const selected={...s,expiry};
 const allStrikes=[...new Set(data.groups.map(g=>g.strike))].sort((a,b)=>b-a);
 const spot=state?.symbol===symbol?state.ticks?.[symbol]?.price:null;
 const center=s.strike??(finite(spot)?spot:allStrikes[Math.floor(allStrikes.length/2)]??0);
 const strikes=[...allStrikes].sort((a,b)=>Math.abs(a-center)-Math.abs(b-center)).slice(0,25).sort((a,b)=>b-a);
 const max=Math.max(0,...data.groups.filter(g=>strikes.includes(g.strike)).map(g=>g[s.metric]??0));
 const horizon=state?.symbol===symbol?state.optionSnapshotStatus?.horizons?.find(h=>h.mode==='DAY'):undefined;
 const structure=horizon?.structure;
 const modelValid=horizon&&finite(horizon.observedMs)&&horizon.observedMs<=asof&&dateET(horizon.observedMs)===dateET(asof)&&structure?.complete;
 const levels=modelValid?[{price:structure?.callWall?.strike,name:'Call Wall'},{price:structure?.magnet,name:'Main Magnet'},{price:structure?.putWall?.strike,name:'Put Wall'}].filter((l):l is {price:number;name:string}=>finite(l.price)&&l.price>0):[];
 const disconnected=!active||state?.options?.phase!=='streaming';
 const historical=now-asof>15000;
 const heading=s.view==='levels'?'MAPA DE NIVELES':'FLUJO POR STRIKE';
 const diagnostics=usable?Object.entries(r.diagnostics??{}).filter(([,n])=>finite(n)&&n>0).map(([k,v])=>`${esc(k)}: ${number(v)}`).join(' · '):'';
 const header=`<header class="tm-header"><div><small>SNIPERS / LEVEL ENGINE</small><h2>${heading}</h2><strong>${esc(symbol)} · ${finite(spot)?price(spot):'Precio pendiente'}</strong></div><div><nav aria-label="Mapas Snipers"><button data-map-view="levels" aria-pressed="${s.view==='levels'}">Mapa de niveles</button><button data-map-view="flow" aria-pressed="${s.view==='flow'}">Flujo por strike</button></nav><small>${clock(asof)} ET · ${historical?'CAPTURA HISTÓRICA':disconnected?'DESCONECTADO / HISTÓRICO':'MUESTRA OBSERVADA'} · cobertura de mercado no certificada</small></div></header>`;
 const controls=`<div class="tm-controls"><label>Vencimiento <select data-map-expiry><option value="">${expiry?'Seleccionar':'Sin operaciones disponibles'}</option>${data.expiries.map(e=>`<option value="${e}" ${e===expiry?'selected':''}>${e}</option>`).join('')}</select></label><label>Población <select data-map-category>${['all',...categories].map(c=>`<option value="${c}" ${s.category===c?'selected':''}>${categoryNames[c]}</option>`).join('')}</select></label><button data-map-metric="contracts" aria-pressed="${s.metric==='contracts'}">Contratos</button><button data-map-metric="premium" aria-pressed="${s.metric==='premium'}">Prima USD</button></div>`;
 let body='';
 if(s.view==='flow')body=`${controls}<div class="tm-flow-layout"><div><div class="tm-heats">${heatChart(data.groups,'call',s.metric,strikes,asof,max,selected)}${heatChart(data.groups,'put',s.metric,strikes,asof,max,selected)}</div><p class="tm-note">${s.metric==='contracts'?'Contratos':'Prima USD'} ejecutados por minuto · escala lineal común 0–${s.metric==='premium'?money(max):number(max)} · ${strikes.length}/${allStrikes.length} strikes · trama: sin dato verificable, no cero.</p>${!data.groups.length?'<p class="tm-empty">Esperando el ledger de ejecuciones. No se reconstruye desde listas recientes truncadas.</p>':''}</div>${inspector(data.groups,selected)}</div>`;
 else body=`<div class="tm-level-layout"><div><h3>PRECIO Y REACCIÓN</h3>${levelChart(state?.symbol===symbol?state:undefined,asof,levels)}${!levels.length?'<p class="tm-note">Walls y Main Magnet pendientes de estructura verificable de esta sesión.</p>':''}</div><aside class="tm-inspector"><h3>QUÉ PASA EN CADA NIVEL</h3>${levels.length?levels.map(l=>`<section><h4>${price(l.price)} <small class="tm-model">${l.name} · modelo</small></h4><dl><dt>Liquidez esperando</dt><dd>—</dd><dt>Órdenes retiradas</dt><dd>—</dd></dl><button data-map-strike="${l.price}" data-map-view="flow">Ver opciones en este strike →</button></section>`).join(''):'<p>Selecciona un strike en Flujo por strike para inspeccionarlo.</p>'}<p class="tm-depth">Profundidad pendiente de fuente. Massive no ofrece Level 2. NBBO y GEX no sustituyen el libro.</p></aside></div>${controls}<h3>OPCIONES EJECUTADAS · 5 INTERVALOS DE 1 MIN</h3>${activity(data.groups,asof)}<p class="tm-note">Absorción y escenario: NO EVALUABLES con este mapa. GEX conserva su modelo de signo; no es inventario observado de dealers.</p>`;
 return `<section class="panel two-maps" data-two-maps>${header}${body}<footer>${usable?esc(r.population):'Ledger todavía no disponible en esta versión de la fuente.'} · vencimiento ${esc(expiry||'pendiente')}<br>Prima bruta; no identifica apertura/cierre. Color = actividad, no dirección futura.${data.rejected?`<br>Filas rechazadas o ambiguas: ${data.rejected}`:''}${diagnostics?`<br>Diagnóstico: ${diagnostics}`:''}${usable&&r.storageError?`<br>Persistencia pendiente: ${esc(r.storageError)}`:''}</footer></section>`;
}
function activity(groups:Group[],asof:number):string {
 const sums=new Map<number,{call:number|null;put:number|null}>();
 const start=Math.floor(asof/60000)*60000-4*60000;
 for(const g of groups.filter(g=>g.startMs>=start)){const row=sums.get(g.strike)??{call:null,put:null};const side=g.kind as 'call'|'put';row[side]=(row[side]??0)+g.contracts;sums.set(g.strike,row);}
 const rows=[...sums].sort((a,b)=>(b[1].call??0)+(b[1].put??0)-(a[1].call??0)-(a[1].put??0)).slice(0,8),max=Math.max(1,...rows.flatMap(([,r])=>[r.call??0,r.put??0]));
 const bar=(n:number|null)=>n==null?'—':`<i style="width:${n/max*100}%"></i>${number(n)}`;
 return rows.length?`<div class="tm-activity">${rows.map(([strike,r])=>`<button data-map-strike="${strike}" data-map-view="flow"><b>${price(strike)}</b><span>CALL ${bar(r.call)}</span><span>PUT ${bar(r.put)}</span></button>`).join('')}</div><p class="tm-note">Cinco intervalos de un minuto; el último puede estar en curso. — = sin dato observado.</p>`:'<p class="tm-empty">Sin cantidades verificables para esta ventana y vencimiento.</p>';
}
export function bindTwoMaps(refresh:()=>void):void {
 const root=document.querySelector<HTMLElement>('[data-two-maps]');if(!root)return;
 const handle=(target:Element)=>{
  const b=target.closest<HTMLElement>('[data-map-view],[data-map-metric],[data-map-strike]');if(!b)return;
  if(b.dataset.mapView==='levels'||b.dataset.mapView==='flow')selection.view=b.dataset.mapView;
  if(b.dataset.mapMetric==='contracts'||b.dataset.mapMetric==='premium')selection.metric=b.dataset.mapMetric;
  if(b.dataset.mapStrike!=null){const n=Number(b.dataset.mapStrike);if(finite(n)&&n>0)selection.strike=n;selection.minute=b.dataset.mapMinute==null?null:Number(b.dataset.mapMinute);selection.side=b.dataset.mapSide==='call'||b.dataset.mapSide==='put'?b.dataset.mapSide:undefined;}
  refresh();
 };
 root.onclick=e=>{if(e.target instanceof Element)handle(e.target);};
 root.onkeydown=e=>{if((e.key==='Enter'||e.key===' ')&&e.target instanceof SVGElement){e.preventDefault();handle(e.target);}};
 root.onchange=e=>{const t=e.target;if(!(t instanceof HTMLSelectElement))return;if(t.hasAttribute('data-map-expiry')){selection.expiry=t.value;selection.strike=null;selection.minute=null;}if(t.hasAttribute('data-map-category'))selection.category=t.value;refresh();};
}
