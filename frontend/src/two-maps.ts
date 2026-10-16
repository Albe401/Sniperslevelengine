import type {LiveState} from './live';
import {escapeHtml as esc} from './model';
import {tradingMapBars,setTradingMapModel,bindTradingMap,tradingMapAction} from './two-maps-chart';
import './two-maps-professional.css';

export interface FlowCell {ticker:string;expiration:string;kind:string;strike:number;startMs:number;endMs:number;category:string;contracts:number;executions:number;verifiedPremium:number;premium:number|null;missingPremiumExecutions:number}
export interface FlowMapReport {storageError?:string|null;method:string;symbol:string;date:string|null;asofMs:number;connected:boolean;firstReceivedMs:number|null;lastReceivedMs:number|null;population:string;status:string;receptionCoveragePct:number|null;cells:FlowCell[];diagnostics:Record<string,number>}
export interface MapSelection {symbol:string;view:'levels'|'flow';metric:'contracts'|'premium';expiry:string;category:string;strike:number|null;minute:number|null;side?:'call'|'put';barMinutes?:number;heatSide?:'both'|'call'|'put';expanded?:boolean}
export const newSelection=(symbol:string):MapSelection=>({symbol,view:'flow',metric:'contracts',expiry:'',category:'all',strike:null,minute:null});
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
function inspector(groups:Group[],s:MapSelection):string {
 const matches=groups.filter(g=>g.strike===s.strike&&(s.minute==null||g.startMs===s.minute)&&(!s.side||g.kind===s.side));
 const chosen=s.minute==null?['call','put'].flatMap(side=>matches.filter(g=>g.kind===side).sort((a,b)=>a.startMs-b.startMs).slice(-1)):matches;
 if(!chosen.length){
  const rows=new Map<number,{call:number|null;put:number|null}>();
  for(const g of groups){const row=rows.get(g.strike)??{call:null,put:null};const k=g.kind as 'call'|'put';row[k]=(row[k]??0)+g.contracts;rows.set(g.strike,row);}
  return `<aside class="tm-inspector"><h3>ACTIVIDAD POR STRIKE</h3><small>Contratos observados · últimos 30 min</small><div class="tm-strike-head"><span>STRIKE</span><span>CALL</span><span>PUT</span></div>${[...rows].sort((a,b)=>(b[1].call??0)+(b[1].put??0)-(a[1].call??0)-(a[1].put??0)).slice(0,12).map(([strike,r])=>`<button class="tm-strike-row" data-map-strike="${strike}"><b>${price(strike)}</b><span>${r.call==null?'—':number(r.call)}</span><span>${r.put==null?'—':number(r.put)}</span></button>`).join('')}<p>Selecciona una celda del calor para ver minuto, prima y ejecuciones.</p><small>CALL: franja superior al strike.<br>PUT: franja inferior. Sin dato: transparente.</small></aside>`;
 }
 return `<aside class="tm-inspector"><h3>STRIKE ${price(s.strike!)}</h3>${chosen.slice(-4).map(g=>`<section><h4>${g.kind.toUpperCase()} · ${clock(g.startMs)} ET</h4><dl><dt>Contratos observados</dt><dd>${number(g.contracts)}</dd><dt>Prima bruta</dt><dd>${money(g.premium)}</dd><dt>Ejecuciones</dt><dd>${number(g.executions)}</dd></dl><small>${g.categories.map(c=>esc(categoryNames[c])).join(' · ')}</small></section>`).join('')}<p>Lado agresor sin clasificar en este mapa.</p><button data-map-view="levels">Ver nivel ${price(s.strike!)} →</button></aside>`;
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
 const strikes=allStrikes;
 const max=Math.max(0,...data.groups.filter(g=>strikes.includes(g.strike)).map(g=>g[s.metric]??0));
 const horizon=state?.symbol===symbol?state.optionSnapshotStatus?.horizons?.find(h=>h.mode==='DAY'):undefined;
 const structure=horizon?.structure;
 const modelValid=horizon&&finite(horizon.observedMs)&&horizon.observedMs<=asof&&dateET(horizon.observedMs)===dateET(asof)&&structure?.complete;
 const levels=modelValid?[{price:structure?.callWall?.strike,name:'Call Wall'},{price:structure?.magnet,name:'Main Magnet'},{price:structure?.putWall?.strike,name:'Put Wall'}].filter((l):l is {price:number;name:string}=>finite(l.price)&&l.price>0):[];
 const disconnected=!active||state?.options?.phase!=='streaming';
 const historical=now-asof>15000;
 const heading=s.view==='levels'?'MAPA DE NIVELES':'FLUJO POR STRIKE';
 const diagnostics=usable?Object.entries(r.diagnostics??{}).filter(([,n])=>finite(n)&&n>0).map(([k,v])=>`${esc(k)}: ${number(v)}`).join(' · '):'';
 const header=`<header class="tm-header"><div class="tm-instrument"><b>${esc(symbol)}</b><strong>${finite(spot)?price(spot):'—'}</strong><span class="tm-session ${disconnected||historical?'is-delayed':''}">${historical?'CAPTURA HISTÓRICA':disconnected?'DESCONECTADO / HISTÓRICO':'MUESTRA OBSERVADA'} · ${clock(asof)} ET</span></div><nav aria-label="Mapas Snipers"><button data-map-view="levels" aria-pressed="${s.view==='levels'}">Mapa de niveles</button><button data-map-view="flow" aria-pressed="${s.view==='flow'}">Flujo por strike</button></nav></header>`;
 const controls=`<div class="tm-controls"><label>Vencimiento <select data-map-expiry><option value="">${expiry?'Seleccionar':'Sin operaciones disponibles'}</option>${data.expiries.map(e=>`<option value="${e}" ${e===expiry?'selected':''}>${e}</option>`).join('')}</select></label><label>Población <select data-map-category>${['all',...categories].map(c=>`<option value="${c}" ${s.category===c?'selected':''}>${categoryNames[c]}</option>`).join('')}</select></label><button data-map-metric="contracts" aria-pressed="${s.metric==='contracts'}">Contratos</button><button data-map-metric="premium" aria-pressed="${s.metric==='premium'}">Prima USD</button></div>`;
 const bars=tradingMapBars(state?.symbol===symbol?state:undefined,asof,s.barMinutes??1);
 setTradingMapModel({symbol,asof,bars,groups:data.groups,levels,selection:selected});
 const tools=`<div class="tm-chart-tools"><div>${[1,2,5].map(n=>`<button data-map-interval="${n}" aria-pressed="${(s.barMinutes??1)===n}">${n}m</button>`).join('')}<span class="tm-tool-separator"></span><button data-map-chart-action="fit" title="Ajustar toda la sesión">Ajustar</button><button data-map-chart-action="follow" title="Volver al último precio">Seguir precio</button></div><div>${s.view==='flow'?(['both','call','put'] as const).map(k=>`<button data-map-layer="${k}" aria-pressed="${(s.heatSide??'both')===k}">${k==='both'?'CALL + PUT':k.toUpperCase()}</button>`).join(''):''}<button data-map-chart-action="expand">${s.expanded?'Reducir':'Ampliar'} ⤢</button></div></div>`;
 const chart=`<div class="tm-chart-surface"><div class="tm-chart-caption"><span>${esc(symbol)} · ${s.barMinutes??1}m · ${s.view==='flow'?'Ejecuciones por strike':'Estructura de opciones'}</span><span data-map-ohlc>Arrastra para desplazar · rueda para zoom</span></div><div id="snipers-trading-map" data-chart-preserve role="img" aria-label="Gráfica interactiva de velas, niveles y flujo por strike"></div>${!bars.length?'<div class="tm-chart-wait">Esperando velas OHLC verificadas del activo</div>':''}<div class="tm-chart-legend"><span><i class="call"></i> CALLS</span><span><i class="put"></i> PUTS</span><span>Color: actividad ejecutada · ${s.metric==='contracts'?'contratos':'prima USD'} · escala lineal común ${max?`0–${s.metric==='premium'?money(max):number(max)}`:'pendiente'}</span></div></div>`;
 let body='';
 const side=s.view==='flow'?inspector(data.groups,selected):`<aside class="tm-inspector"><h3>NIVELES DE OPCIONES</h3>${levels.length?levels.map(l=>`<section><small>${esc(l.name)} · modelo</small><h4>${price(l.price)}</h4><button data-map-strike="${l.price}" data-map-view="flow">Inspeccionar flujo →</button></section>`).join(''):'<p>Walls y Main Magnet pendientes de estructura verificable de esta sesión.</p>'}<div class="tm-data-state"><span>Profundidad de mercado</span><b>Sin fuente L2</b><small>Massive no ofrece Level 2. Órdenes pendientes y retiradas: —</small></div></aside>`;
 body=`${tools}${controls}<div class="tm-terminal">${chart}${side}</div><div class="tm-statusbar"><span>${s.view==='flow'?'Celdas por minuto y strike':'Niveles modelados'} · ${strikes.length} strikes observados</span><span>${esc(expiry||'Vencimiento pendiente')} · USD · ET</span></div>`;
 return `<section class="panel two-maps tm-professional ${s.expanded?'tm-expanded':''}" data-two-maps aria-label="${heading}">${header}${body}<details class="tm-method"><summary>Cobertura y metodología${usable&&r.storageError?' · error de persistencia':''}</summary><p>${usable?esc(r.population):'Ledger todavía no disponible en esta versión de la fuente. Esperando el ledger de ejecuciones.'} · sin dato verificable, no cero.</p><p>Prima bruta; no identifica apertura/cierre. Color = actividad, no dirección futura. Absorción y escenario: NO EVALUABLES con este mapa. GEX es un modelo, no inventario observado de dealers. Profundidad pendiente: Massive no ofrece Level 2.</p>${s.view==='levels'?activity(data.groups,asof):''}${data.rejected?`<p>Filas rechazadas o ambiguas: ${data.rejected}</p>`:''}${diagnostics?`<p>Diagnóstico: ${diagnostics}</p>`:''}${usable&&r.storageError?`<p>Persistencia pendiente: ${esc(r.storageError)}</p>`:''}</details></section>`;
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
 bindTradingMap((strike,minute,side)=>{selection.strike=strike;selection.minute=minute;selection.side=side;refresh();});
 const handle=(target:Element)=>{
  const b=target.closest<HTMLElement>('[data-map-view],[data-map-metric],[data-map-strike],[data-map-interval],[data-map-layer],[data-map-chart-action]');if(!b)return;
  if(b.dataset.mapChartAction){if(b.dataset.mapChartAction==='expand'){selection.expanded=!selection.expanded;refresh();}else tradingMapAction(b.dataset.mapChartAction);return;}
  if(b.dataset.mapInterval)selection.barMinutes=Number(b.dataset.mapInterval);
  if(['both','call','put'].includes(b.dataset.mapLayer??''))selection.heatSide=b.dataset.mapLayer as 'both'|'call'|'put';
  if(b.dataset.mapView==='levels'||b.dataset.mapView==='flow')selection.view=b.dataset.mapView;
  if(b.dataset.mapMetric==='contracts'||b.dataset.mapMetric==='premium')selection.metric=b.dataset.mapMetric;
  if(b.dataset.mapStrike!=null){const n=Number(b.dataset.mapStrike);if(finite(n)&&n>0)selection.strike=n;selection.minute=b.dataset.mapMinute==null?null:Number(b.dataset.mapMinute);selection.side=b.dataset.mapSide==='call'||b.dataset.mapSide==='put'?b.dataset.mapSide:undefined;}
  refresh();
 };
 root.onclick=e=>{if(e.target instanceof Element)handle(e.target);};
 root.onkeydown=e=>{if((e.key==='Enter'||e.key===' ')&&e.target instanceof SVGElement){e.preventDefault();handle(e.target);}};
 root.onchange=e=>{const t=e.target;if(!(t instanceof HTMLSelectElement))return;if(t.hasAttribute('data-map-expiry')){selection.expiry=t.value;selection.strike=null;selection.minute=null;}if(t.hasAttribute('data-map-category'))selection.category=t.value;refresh();};
}
