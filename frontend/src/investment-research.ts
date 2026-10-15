import {isTauri} from '@tauri-apps/api/core';

import {escapeHtml} from './model';

import type {Company} from './investment';



export interface Zone{center:number;low:number;high:number;pivots:number;evidence:string[]}

export interface Zones{supports:Zone[];resistances:Zone[];sessions:number;atr:number|null}

export function zonePanel(c:Company,compact=false):string{

 const z=c.zones;

 if(!z||z.atr==null)return `<p class="ir-footnote">Zonas históricas: ${z?'se requieren al menos 200 sesiones diarias válidas':'vuelve a explorar empresas para calcularlas'}.</p>`;

 const card=(a:Zone,label:string,side:'supports'|'resistances')=>{

  const r=c.research;

  const valid=r&&r.referenceDate===c.priceDate&&r.referencePrice===m(c,'last_close')&&Date.now()>=r.fetchedMs&&Date.now()-r.fetchedMs<=86400000;

  const hits=valid?r.structure[side].filter(l=>l.strike>=a.low&&l.strike<=a.high):[];

  return `<article class="ir-level ${side==='supports'?'support':'resistance'}"><span>${label}</span><strong>${n(a.low,true)} – ${n(a.high,true)}</strong><b>Centro ${n(a.center,true)}</b><small>${a.evidence.map(e).join(' · ')}</small><small>${hits.length?`Coincide con OI ${side==='supports'?'PUT':'CALL'}: ${hits.map(l=>n(l.strike,true)).join(', ')}${r?.chainComplete?'':' · muestra parcial'}`:'Sin coincidencia verificada con los strikes OI mostrados'}</small></article>`;

 };

 return `<section class="ir-price-zones"><span class="ir-eyebrow">ZONAS HISTÓRICAS DIARIAS · ${e(c.priceDate)}</span><div class="ir-level-grid">${(['supports','resistances'] as const).map(side=>{const list=compact?z[side].slice(0,1):z[side];return list.map((a,i)=>card(a,`${side==='supports'?'Soporte':'Resistencia'} ${i+1}`,side)).join('')||`<p class="ir-footnote">Sin ${side==='supports'?'soportes':'resistencias'} históricos confirmados.</p>`;}).join('')}</div>${compact?'':`<p class="ir-footnote">Hasta tres zonas cercanas por lado · pivotes de las últimas 260 sesiones, confirmados con tres sesiones posteriores · bandas según ATR 14 (media simple). EMA 50/200 y SMA 200 son coincidencias, no probabilidades ni identificación institucional. ${z.sessions} sesiones disponibles. Sin niveles de relleno.</p>`}</section>`;

}



export interface Level{strike:number;openInterest:number;relativeStrength:number;strength:string;expirations:string[];contracts:number}
export function keyMetrics(c:Company):string {
 const fields=[['return_on_equity','ROE','%','Rentabilidad sobre patrimonio'],['beta','Beta · 1 año / SPY','','Sensibilidad frente a SPY'],['price_to_earnings','P/E ratio · TTM','×','Precio / beneficios históricos'],['forward_pe','P/E forward','×','Consenso de 4 trimestres pendientes']];
 return `<section class="ir-key-metrics" aria-label="Métricas clave de la compañía">${fields.map(([id,label,unit,description])=>{
  const metric=c.metrics.find(v=>v.id===id);const valid=metric?.value!=null&&Number.isFinite(metric.value);
  const reason=valid?description:!metric?'Vuelve a explorar empresas':id==='forward_pe'&&/403|401|sin permiso/.test(metric.source)?'Fuente de estimaciones sin acceso':id==='forward_pe'?'Sin estimaciones verificadas':'Sin datos suficientes';
  return `<article class="ir-key-metric ${valid?'':'pending'}"><span>${e(label)}</span><strong>${valid?n(metric!.value)+unit:'—'}</strong><small>${e(reason)}</small><details><summary>Fuente y cálculo</summary><p>${e(metric?.source??'Pendiente de actualizar el análisis')}</p></details></article>`;
 }).join('')}</section>`;
}
export interface Block{id?:string;price:number;shares:number;notional:number;executionMs:number|null;sipMs:number;trfMs:number|null;trfId:number;conditions:number[];nbbo:string;bid:number|null;ask:number|null;quoteMs:number|null}

export interface Research{ticker:string;fetchedMs:number;referencePrice:number;referenceDate:string;expirationFrom:string;expirationTo:string;chainComplete:boolean;chainRows:number;structure:{supports:Level[];resistances:Level[];dominantSupport:Level|null;dominantResistance:Level|null;accepted:number;rejected:number};tradeDate:string;session:string;tradeComplete:boolean;tradesRequested?:boolean;examinedTrades:number;fromMs:number;throughMs:number;minimumNotional:number;blocks:Block[];errors:string[]}

const e=(v:unknown)=>escapeHtml(String(v??''));

const n=(v:number|null|undefined,dollar=false)=>v==null||!Number.isFinite(v)?'—':new Intl.NumberFormat('en-US',{maximumFractionDigits:2,...(dollar?{style:'currency',currency:'USD'}:{})}).format(v);

const time=(ms:number|null)=>ms?new Date(ms).toLocaleString('es',{timeZone:'America/New_York',hour12:false})+' ET':'No disponible';

const m=(c:Company,k:string)=>c.metrics.find(m=>m.id===k)?.value??null;

export function distance(c:Company,id:string):number|null{const p=m(c,'last_close'),v=m(c,id);return p!=null&&v!=null&&p>0&&v>0?(p/v-1)*100:null;}

export function matchesEma(c:Company,filter:string,threshold:number):boolean{if(!filter)return true;const ids=filter==='either'?['ema50','ema200']:[filter];return ids.some(id=>{const d=distance(c,id);return d!==null&&Math.abs(d)<=threshold;});}

export function emaCards(c:Company):string{return `<div class="ir-ma-grid">${[['ema50','EMA 50 · diaria'],['ema200','EMA 200 · diaria'],['sma200','SMA 200 · diaria']].map(([id,label])=>{const d=distance(c,id);return `<article class="ir-ma ${d==null?'':d>=0?'above':'below'}"><span>${label}</span><strong>${n(m(c,id),true)}</strong><b>${d==null?'Actualizar análisis':d>0?'Precio por encima':d<0?'Precio por debajo':'Precio sobre la media'}</b><small>${d==null?'Histórico insuficiente o informe anterior':`${d>0?'+':''}${n(d)}% de distancia`} · cierre ${e(c.priceDate)}</small></article>`;}).join('')}</div>`;}

function levelCard(l:Level|undefined,label:string,kind:string){return `<article class="ir-level ${kind}"><span>${e(label)}</span><strong>${n(l?.strike,true)}</strong><b>${l?e(l.strength)+' · '+n(l.relativeStrength)+'% relativo':'Sin concentración verificada'}</b><small>OI ${n(l?.openInterest)} · ${l?l.contracts:0} contratos distintos</small>${l?`<details><summary>Vencimientos (${l.expirations.length})</summary><p>${l.expirations.map(e).join(' · ')}</p></details>`:''}</article>`;}

export function researchPanel(c:Company,busy:string):string{

 const r=c.research;const stale=r&&Date.now()-r.fetchedMs>86400000;const s=r?.structure;

 return `${zonePanel(c)}<section class="ir-research"><header><div><span class="ir-eyebrow">ESTRUCTURA DE OPCIONES · INVESTIGACIÓN</span><h2>Concentraciones de opciones · evidencia adicional</h2></div></header><p class="ir-footnote">Concentraciones de interés abierto como referencia potencial; no identifican al inversor ni garantizan defensa del nivel. Tres mayores concentraciones PUT bajo el cierre y CALL sobre el cierre, ordenadas por proximidad.</p>

 <div class="ir-research-controls"><label>Fecha de operaciones<input data-ir-trade-date type="date" value="${e(r?.tradeDate??'')}"></label><label>Franja de reporte<select data-ir-trade-session>${[['post','Después del cierre'],['pre','Antes de apertura'],['regular','Sesión regular'],['all','04:00–20:00 ET']].map(([v,l])=>`<option value="${v}" ${(r?.session??'post')===v?'selected':''}>${l}</option>`).join('')}</select></label><label>Importe mínimo<select data-ir-trade-min>${[100000,500000,1000000,5000000].map(v=>`<option value="${v}" ${(r?.minimumNotional??1000000)===v?'selected':''}>${n(v,true)}</option>`).join('')}</select></label><button data-ir-research="${e(c.ticker)}" ${!isTauri()||busy?'disabled':''}>${busy===c.ticker?'Consultando Massive…':r?'Actualizar estructura y operaciones':'Consultar estructura y operaciones'}</button></div>

 <p class="ir-footnote">Fecha vacía: última sesión iniciada. La fecha y franja filtran el reporte SIP de acciones; la cadena de opciones siempre es la disponible al consultar.</p>

 ${r?`<p class="ir-footnote">Consulta ${time(r.fetchedMs)} ${stale?'· ACTUALIZACIÓN PENDIENTE':''}<br>Referencia: cierre ${n(r.referencePrice,true)} del ${e(r.referenceDate)} · vencimientos ${e(r.expirationFrom)} a ${e(r.expirationTo)} · ${r.chainRows} contratos recibidos, ${s?.accepted} válidos con OI.</p>${!r.chainComplete?'<p class="ir-error">Cadena parcial o no disponible: niveles provisionales de la muestra, sin dominante confirmado del universo.</p>':''}${r.errors.map(x=>`<p class="ir-error">${e(x)}</p>`).join('')}`:'<p class="ir-footnote">Carga bajo demanda por empresa; no añade conexiones en vivo ni cambia el activo del motor intradía.</p>'}

 <div class="ir-dominants">${levelCard(s?.dominantSupport??undefined,r?.chainComplete?'PUT dominante por OI':'Mayor soporte observado · provisional','support')}${levelCard(s?.dominantResistance??undefined,r?.chainComplete?'CALL dominante por OI':'Mayor resistencia observada · provisional','resistance')}</div>

 <div class="ir-level-grid">${[0,1,2].map(i=>levelCard(s?.supports[i],`Concentración PUT ${i+1}`,'support')).join('')}${[0,1,2].map(i=>levelCard(s?.resistances[i],`Concentración CALL ${i+1}`,'resistance')).join('')}</div>

 <p class="ir-footnote">Fuerza relativa al mayor OI de cada lado: extremo ≥80%, alto ≥50%, medio ≥20%, bajo &lt;20%. OI corresponde al cierre anterior, no a dinero entrando ahora. La ventana cubre hasta un año de vencimientos.</p>

 <section class="ir-dark"><header><div><span class="ir-eyebrow">DARK POOL / FUERA DE BOLSA</span><h2>Grandes operaciones reportadas · ${e(c.ticker)}</h2></div><span class="ir-badge">${r&&r.tradesRequested!==false?e(r.tradeDate):'Pendiente de consulta'}</span></header><p class="ir-footnote">Reportes FINRA TRF: incluyen ATS y operaciones fuera de bolsa de otros tipos. No confirman institución, compra o venta. Pueden ocurrir también durante la sesión regular. Hora ejecutada y hora reportada se muestran por separado.</p>

 ${r&&r.tradesRequested!==false?`<p class="ir-footnote">${time(r.fromMs)} → ${time(r.throughMs)} · ${r.examinedTrades.toLocaleString()} operaciones bursátiles examinadas · mínimo ${n(r.minimumNotional,true)} · ${r.tradeComplete?'Paginación completa de esta franja':'MUESTRA PARCIAL: últimas 30.000 operaciones como máximo'} · hasta 12 mayores reportes TRF válidos.</p>`:''}

 <div class="ir-block-grid">${r?.blocks.map(b=>`<article class="ir-block"><header><b>${e(c.ticker)} <span>REPORTE TRF ${b.trfId}</span></b><span class="ir-badge">${e(b.nbbo)}</span></header><strong>${n(b.notional,true)}</strong><p>${n(b.shares)} acciones × ${n(b.price,true)}</p><dl><div><dt>Ejecución</dt><dd>${time(b.executionMs)}</dd></div><div><dt>Reporte SIP</dt><dd>${time(b.sipMs)}</dd></div><div><dt>Recepción TRF</dt><dd>${time(b.trfMs)}</dd></div><div><dt>Demora de reporte</dt><dd>${b.executionMs==null?'—':n((b.sipMs-b.executionMs)/1000)+' s'}</dd></div><div><dt>Bid / Ask histórico</dt><dd>${n(b.bid,true)} / ${n(b.ask,true)}</dd></div><div><dt>Hora NBBO</dt><dd>${time(b.quoteMs)}</dd></div><div><dt>Condiciones</dt><dd>${b.conditions.join(', ')||'Sin códigos'}</dd></div></dl><small>Posición respecto al NBBO, no sentimiento confirmado. Correcciones conocidas excluidas.</small></article>`).join('')||`<div class="ir-empty">${r&&r.tradesRequested!==false?'No se observaron bloques válidos por encima del importe mínimo en la muestra consultada.':'Los niveles se cargan automáticamente. Las operaciones fuera de bolsa se consultan al abrir la ficha; puedes reintentar con Consultar estructura y operaciones.'}</div>`}</div>

 <p class="ir-footnote">Las alertas «Golden Sweep» de la primera referencia son opciones. Este bloque muestra acciones; no atribuye apertura de posición, sweep o sentimiento sin evidencia verificable.</p></section></section>`;

}

