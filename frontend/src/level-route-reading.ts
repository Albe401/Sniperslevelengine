import type {LiveState} from './live';
import type {Group,MapSelection,FlowMapReport} from './two-maps';
import type {AuctionReading} from './auction-map-panel';
import {escapeHtml as esc} from './model';
import {reportOiHeatmap} from './option-oi-heatmap-report';
import type {EtfReport} from './etf-dealers';

const finite=(x:unknown):x is number=>typeof x==='number'&&Number.isFinite(x);
const n=(x:number|null|undefined)=>x==null?'—':x.toLocaleString('es-ES',{maximumFractionDigits:0});
const px=(x:number|null|undefined)=>x==null?'—':x.toFixed(2).replace('.',',');
const pct=(x:number|null)=>x==null?'—':`${x.toFixed(1)}%`;
export const routeClock=(t:number)=>new Date(t).toLocaleTimeString('es-ES',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit',hour12:false});
export interface RouteActivity {strike:number;current:number;previous:number;premium:number|null;share:number|null;previousShare:number|null;currentObserved:boolean;previousObserved:boolean}
/** Each stripe ends at its displayed minute + 1: five CLOSED minutes, neutral CALL+PUT quantity. */
export function routeHeatCells(groups:Group[],asof:number,metric:'contracts'|'premium'='contracts'){
 const end=Math.floor(asof/60000)*60000,rows=new Map<string,{g:Group;value:number|null}>();
 for(const g of groups){if(g.startMs>=end)continue;for(let offset=0;offset<5;offset++){
  const minute=g.startMs+offset*60000;if(minute>=end||minute<end-1800000)continue;
  const key=g.strike+':'+minute,old=rows.get(key),value=metric==='contracts'?g.contracts:g.premium;if(old)old.value=old.value==null||value==null?null:old.value+value;else rows.set(key,{g:{...g,startMs:minute},value});
 }}
 return [...rows.values()].filter((x):x is {g:Group;value:number}=>finite(x.value)&&x.value>=0&&(metric==='premium'||Number.isSafeInteger(x.value)));
}
/** Two adjacent CLOSED five-minute windows. Values are received executions, never whole-market coverage. */
export function routeActivity(groups:Group[],asof:number):RouteActivity[]{
 const end=Math.floor(asof/60000)*60000,start=end-300000,previous=start-300000;
 const rows=new Map<number,RouteActivity>();let total=0,prior=0;
 for(const g of groups){if(g.startMs<previous||g.startMs>=end)continue;
  const r=rows.get(g.strike)??{strike:g.strike,current:0,previous:0,premium:0,share:null,previousShare:null,currentObserved:false,previousObserved:false};
  if(g.startMs>=start){r.current+=g.contracts;r.currentObserved=true;total+=g.contracts;r.premium=r.premium==null||g.premium==null?null:r.premium+g.premium;}
  else{r.previous+=g.contracts;r.previousObserved=true;prior+=g.contracts;}rows.set(g.strike,r);
 }
 return [...rows.values()].filter(r=>Number.isSafeInteger(r.current)&&Number.isSafeInteger(r.previous)&&(r.premium==null||finite(r.premium))).map(r=>({...r,premium:r.currentObserved?r.premium:null,share:total>0?r.current/total*100:null,previousShare:prior>0?r.previous/prior*100:null})).sort((a,b)=>b.current-a.current||a.strike-b.strike);
}
export function routeAuction(state:LiveState|undefined,symbol:string,asof:number):AuctionReading|undefined{
 const a=state?.symbol===symbol?state.auctionMap:undefined;
 if(!a||a.symbol!==symbol||!finite(a.asOfMs)||a.asOfMs>asof||asof-a.asOfMs>300000||!Array.isArray(a.bins)||a.bins.length>5000||a.bins.some(b=>![b.low,b.high,b.volume].every(finite)||b.low<=0||b.high<=b.low||b.volume<0))return;
 return a;
}
export function routeReferences(levels:{price:number;name:string}[],spot:number|null){
 const origin=levels.find(l=>/Main Magnet/.test(l.name))?.price??null;
 const anchor=origin??spot;
 const sorted=[...new Set(levels.map(l=>l.price))].sort((a,b)=>a-b);
 return {origin,upper:anchor==null?null:sorted.find(p=>p>anchor)??null,lower:anchor==null?null:sorted.filter(p=>p<anchor).at(-1)??null};
}
const oiCache=new WeakMap<EtfReport,Map<string,ReturnType<typeof reportOiHeatmap>>>();
export function routeOi(state:LiveState|undefined,symbol:string,strike:number|null,side:'call'|'put',expiry:string,asof:number){
 const chain=(state as (LiveState&{contractChain?:{observation?:{value?:EtfReport}}})|undefined)?.contractChain?.observation?.value;
 const r=chain??state?.optionSnapshotStatus?.structuralEnrichment;
 if(state?.symbol!==symbol||!r||r.symbol!==symbol||!finite(r.at)||r.at>asof||asof-r.at>300000||!r.expirations.includes(expiry)||strike==null)return;
 try{let cache=oiCache.get(r);if(!cache){cache=new Map();oiCache.set(r,cache);}let map=cache.get(expiry);if(!map){map=reportOiHeatmap({...r,expirations:[expiry]});cache.set(expiry,map);}const cell=map.cells.find(c=>c.strike===strike&&c.side===side);return cell?.state==='known'?{oi:cell.oi,date:map.oiDate,at:r.at}:undefined;}catch{return;}
}
/** Reuses existing bid/ask execution buckets, a separate rolling window, never relabelled as the closed heat window. */
export function routeAggression(state:LiveState|undefined,symbol:string,strike:number|null,side:string,expiry:string,category:string,asof:number){
 if(state?.symbol!==symbol||category!=='all'||state.flow?.asof_ms!==asof)return;
 const rows=state.flow.bid_ask?.filter(r=>r.strike===strike&&r.kind===side&&r.expiration===expiry);
 if(!rows?.length||new Set(rows.map(r=>r.ticker)).size!==rows.length||rows.some(r=>!r.covered||[r.ask,r.bid,r.unknown].some(v=>!finite(v)||v<0)))return;
 const ask=rows.reduce((s,r)=>s+r.ask!,0),bid=rows.reduce((s,r)=>s+r.bid!,0),unknown=rows.reduce((s,r)=>s+r.unknown!,0),total=ask+bid+unknown;
 return {buyPct:ask+bid>0?100*ask/(ask+bid):null,classifiedPct:total>0?100*(ask+bid)/total:null,ask,bid,unknown};
}
export function routeSummary(rows:RouteActivity[],asof:number,r:FlowMapReport|undefined){
 const priorStart=Math.floor(asof/60000)*60000-600000;
 // Even an uninterrupted observed sample does not certify full-chain or unchanged membership.
 const comparable=!!r&&r.firstReceivedMs!=null&&r.firstReceivedMs<=priorStart&&!r.storageError&&Object.values(r.diagnostics??{}).every(v=>v===0);
 const changes=comparable?rows.filter(x=>x.currentObserved&&x.previousObserved&&x.share!=null&&x.previousShare!=null).map(x=>({...x,delta:x.share!-x.previousShare!})):[];
 const gain=changes.filter(x=>x.delta>0).sort((a,b)=>b.delta-a.delta||a.strike-b.strike)[0];
 const loss=changes.filter(x=>x.delta<0).sort((a,b)=>a.delta-b.delta||a.strike-b.strike)[0];
 return {comparable,gain,loss,text:gain||loss?[gain?`${px(gain.strike)} gana cuota observada`:null,loss?`${px(loss.strike)} pierde cuota observada`:null].filter(Boolean).join(' · '):rows[0]?.currentObserved?`Mayor volumen observado: ${px(rows[0].strike)} · comparación temporal pendiente`:'Esperando ejecuciones verificables en ventanas cerradas'};
}
export function routeBanner(levels:{price:number;name:string}[],spot:number|null,rows:RouteActivity[],asof:number,r:FlowMapReport|undefined,assetSearch='' ){
 const refs=routeReferences(levels,spot),summary=routeSummary(rows,asof,r);
 return `<div class="tm-route-reading"><div><strong>${spot!=null?`Precio ${px(spot)}`:'Precio pendiente'}${refs.origin!=null?` · Imán ${px(refs.origin)}`:''}</strong><span>${esc(summary.text)}</span></div>${assetSearch}<div class="tm-route-chips"><span>Origen <b>${px(refs.origin)}</b></span><span>Referencia superior <b>${px(refs.upper)}</b></span><span>Referencia inferior <b>${px(refs.lower)}</b></span></div></div>`;
}
export function routeInspector(state:LiveState|undefined,symbol:string,s:MapSelection,groups:Group[],asof:number,levels:{price:number;name:string}[],spot:number|null,r:FlowMapReport|undefined){
 const rows=routeActivity(groups,asof),refs=routeReferences(levels,spot),strike=s.strike??refs.origin??rows[0]?.strike??null;
 const side=s.side??'call',end=Math.floor(asof/60000)*60000,sideRows=routeActivity(groups.filter(g=>g.kind===side),asof),row=sideRows.find(x=>x.strike===strike),summary=routeSummary(rows,asof,r);
 const h=state?.symbol===symbol?state.optionSnapshotStatus?.horizons?.find(h=>h.expirations.length===1&&h.expirations[0]===s.expiry&&h.observedMs<=asof&&asof-h.observedMs<=300000):undefined;
 const structural=h?.structure.complete?h.structure.strikeRows?.filter(x=>x.strike===strike):undefined;
 const sr=structural?.length===1?structural[0]:undefined;
 const inventory=routeOi(state,symbol,strike,side,s.expiry,asof),aggression=routeAggression(state,symbol,strike,side,s.expiry,s.category,asof);
 const oi=inventory?.oi??sr?.[side==='call'?'call_oi':'put_oi'];
 const oiKnown=Number.isSafeInteger(oi)&&oi!>=0;
 const gamma=h?.structure.gexAvailable&&h.exposure.gexComplete?sr?.[side==='call'?'call_gex':'put_gex']:undefined;
 const gammaKnown=finite(gamma)&&gamma>=0;
 const auction=routeAuction(state,symbol,asof);
 const clocks=`${routeClock(end-300000)}–${routeClock(end)} ET`;
 const choices=[...new Map([...levels.map(l=>[l.price,l.name] as const),...rows.slice(0,6).map(x=>[x.strike,'Actividad observada'] as const)]).entries()].sort((a,b)=>b[0]-a[0]);
 return `<aside class="tm-inspector tm-route-inspector"><h3>LECTURA POR NIVEL</h3><h4>${px(strike)} · ${side.toUpperCase()}</h4><small>${esc(s.expiry||'Vencimiento pendiente')} · ${clocks}</small><div class="tm-side-pick"><button data-map-side="call" aria-pressed="${side==='call'}">CALL</button><button data-map-side="put" aria-pressed="${side==='put'}">PUT</button></div><dl><dt>OI de referencia</dt><dd>${oiKnown?n(oi):'—'}</dd><dt>Volumen 5 min observado</dt><dd>${row?.currentObserved?n(row.current):'—'}</dd><dt>Cuota CALL + PUT</dt><dd>${(()=>{const a=rows.find(x=>x.strike===strike);return a?.currentObserved?`${summary.comparable&&a.previousObserved?pct(a.previousShare)+' → ':''}${pct(a.share)}`:'—';})()}</dd><dt>Prima negociada · ${side.toUpperCase()}</dt><dd>${row?.premium==null?'—':row.premium.toLocaleString('es-ES',{style:'currency',currency:'USD',maximumFractionDigits:0})}</dd><dt>Compra estimada · rodante</dt><dd>${aggression?pct(aggression.buyPct):'—'}</dd><dt>Volumen clasificado</dt><dd>${aggression?pct(aggression.classifiedPct):'—'}</dd><dt>Gamma bruta · ${side.toUpperCase()}</dt><dd>${gammaKnown?gamma.toLocaleString('es-ES',{style:'currency',currency:'USD',maximumFractionDigits:0})+' / 1%':'—'}</dd><dt>Fecha efectiva OI</dt><dd>${esc(inventory?.date??'No suministrada')}</dd></dl><small>Agresión: bid/ask, 5 min rodantes hasta ${routeClock(asof)} ET, muestra cubierta; distinta de la ventana cerrada del calor. OI y gamma: ${h?`${esc(h.label)} · captura ${routeClock(h.observedMs)} ET`:'sin referencia del mismo vencimiento'}. No se mezclan horizontes.</small><section><h3>AMT · SUBYACENTE</h3><p>${auction?`POC ${px(auction.pocLow)}–${px(auction.pocHigh)} · ${auction.provisional?'perfil parcial':'perfil observado'} · ${routeClock(auction.asOfMs)} ET`:'Perfil AMT pendiente para esta captura.'}</p><small>Volumen del subyacente, separado del flujo de opciones.</small></section><section><h3>CONDICIÓN DEL ESCENARIO</h3><p>Transición cuantitativa pendiente de entrenamiento y validación.</p><small>Las referencias superior e inferior no indican una dirección prevista. Sin porcentajes de llegada.</small></section><div class="tm-route-level-list"><h3>NIVELES Y ACTIVIDAD</h3>${choices.map(([p,name])=>`<button data-map-strike="${p}" aria-pressed="${p===strike}"><b>${px(p)}</b><span>${esc(name.replace('Main Magnet','Imán principal'))}</span></button>`).join('')}</div></aside>`;
}


