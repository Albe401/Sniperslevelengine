import {blockMemory} from './block-memory';
import type {AtrRead} from './intraday-atr';
import {dailyDistance} from './atr-context';
import {cvdPanel} from './cvd';
import {escapeHtml as esc} from './model';
import type {LiveState} from './live';
import {interpretationFresh} from './confirmations';
import './block-pressure.css';
import {analystTargetsPanel} from './analyst-targets';
import {selectedPrice} from './selected-price';
export interface BlockPeriod {count:number;total:number;buy:number;sell:number;unknown:number;buy_pct:number|null;sell_pct:number|null;coverage_pct:number|null;dominant:string;complete:boolean;finalized:boolean;through_ms:number;top_levels:{price:number;total:number;buy:number;sell:number;count:number}[]}
export interface BlockReport {symbol:string;date:string;minimum_notional:number;premarket:BlockPeriod;regular:BlockPeriod;excluded_time:number}
const selected=new Map<string,'premarket'|'regular'>();
export function defaultBlockPeriod(now:number):'premarket'|'regular'{
 const parts=new Intl.DateTimeFormat('en-US',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit',hourCycle:'h23'}).formatToParts(new Date(now));
 const minutes=Number(parts.find(p=>p.type==='hour')?.value)*60+Number(parts.find(p=>p.type==='minute')?.value);
 return minutes>=570?'regular':'premarket';
}
export function selectBlockPeriod(symbol:string,period:string){if(period==='premarket'||period==='regular')selected.set(symbol,period);}
const money=(n:number)=>new Intl.NumberFormat('en-US',{style:'currency',currency:'USD',notation:'compact',maximumFractionDigits:1}).format(n);
const pct=(n:number|null|undefined)=>n!=null&&Number.isFinite(n)?`${n.toFixed(1)}%`:'—';
export function blockPressurePanel(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now(),atr?:AtrRead,includeTargets=true):string{
 const report=s?.offExchange?.analytics?.blocks;
 const today=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York',year:'numeric',month:'2-digit',day:'2-digit'}).format(new Date(now));
 const usable=active&&s?.symbol===symbol&&report?.symbol===symbol&&report.date===today&&interpretationFresh(s.publishedMs,now);
 const period=selected.get(symbol)??defaultBlockPeriod(now);const retained=blockMemory.get(symbol,now)?.[period];const current=usable?report?.[period]:undefined;const saved=!!retained&&(!current||current.through_ms<retained.through_ms||current.total<retained.total||current.buy+current.sell<retained.buy+retained.sell);const p=saved?retained:current;
 const valid=!!p&&[p.total,p.buy,p.sell,p.unknown].every(n=>Number.isFinite(n)&&n>=0)&&Math.abs(p.total-p.buy-p.sell-p.unknown)<=Math.max(0.01,p.total*1e-8);
 const classified=valid&&p.buy+p.sell>0;
 const buy=classified?p.buy/(p.buy+p.sell)*100:null;const sell=buy==null?null:100-buy;
 const coverage=valid&&p.total>0?(p.buy+p.sell)/p.total*100:null;
 const dominates=valid&&p.complete&&(p.buy+p.sell)>0&&Math.abs(p.buy-p.sell)>p.unknown;
 const label=saved?'Guardado del día · actualizando historial':!usable?'Esperando datos propios del activo':!valid?'Datos de bloques no verificables':!p.count?(p.complete?'Sin bloques ≥ $100.000 observados':'Historial pendiente · aún sin bloques recibidos'):dominates?(p.buy>p.sell?'Domina presión compradora estimada':'Domina presión vendedora estimada'):classified?`${p.complete?'Dominancia no confirmada':'Muestra parcial · dominancia no confirmada'}`:'Dirección sin clasificar';
 return `<section class="block-pressure" aria-label="Bloques de acciones fuera de bolsa"><header><span>BLOQUES · FUERA DE BOLSA</span><nav aria-label="Periodo de bloques">${(['premarket','regular'] as const).map(k=>`<button data-block-period="${k}" data-block-symbol="${esc(symbol)}" aria-pressed="${k===period}">${k==='premarket'?'Premarket':'Sesión'}</button>`).join('')}</nav></header><details class="block-method" data-horizon-details="block-method"><summary>Método y calidad</summary><p>Operaciones individuales ≥ $100.000. Compra/venta agresiva estimada contra bid/ask histórico anterior al evento, hasta 2 s; spread máximo 1%. Medio del spread, cotizaciones anómalas y condiciones ambiguas quedan sin clasificar. Los porcentajes usan solo importe clasificado; no indican probabilidad ni identidad institucional.</p><p>${esc(usable?s?.offExchange.blockQuoteStatus||'Esperando cotizaciones históricas':'Sin conexión vigente')}. ${report&&usable?`${report.excluded_time} operaciones excluidas por hora de ejecución no verificable.`:''} Zonas agrupadas cada $0,25.</p><p>Incluye reportes TRF de distintos tipos; no identifica un dark pool concreto. El histórico puede incorporar correcciones y modificar el resumen.</p></details><div class="block-pressure-sides"><div class="block-buy"><span>BLOQUE DE COMPRAS</span><strong>${pct(buy)}</strong><small>${classified?money(p.buy):'—'}</small></div><div class="block-sell"><span>BLOQUE DE VENTAS</span><strong>${pct(sell)}</strong><small>${classified?money(p.sell):'—'}</small></div></div><div class="block-pressure-bar" aria-label="Participación del importe clasificado">${buy==null?'':`<i style="width:${buy}%"></i>`}</div><p class="block-pressure-lead">${esc(label)}</p><p>Cobertura ${pct(coverage)} · Sin clasificar ${valid?money(p.unknown):'—'}</p><p>${period==='premarket'?'04:00–09:30 ET':'Sesión regular ET'} · ${valid&&p.complete?'Historial recibido':'Historial parcial'}${valid&&p.finalized?' · periodo cerrado':''}</p>${valid&&p.top_levels.length?`<div class="block-pressure-zones">${p.top_levels.slice(0,1).map(z=>`<span>Zona principal $${z.price.toFixed(2)}${selectedPrice(symbol,s,active,now).live?' · '+dailyDistance(atr,selectedPrice(symbol,s,active,now).value!,z.price):''} <small>${money(z.total)} · ${z.count} operaciones</small></span>`).join('')}</div>`:''}${includeTargets?analystTargetsPanel(symbol,s?.ticks&&selectedPrice(symbol,s,active,now).live?selectedPrice(symbol,s,active,now).value:null,undefined,cvdPanel(s?.cvd,symbol,active,period,now,atr),atr):""}</section>`;
}

