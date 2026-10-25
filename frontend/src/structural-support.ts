import type {LiveState} from './live';
import type {PriceStructureCard} from './price-structure';
import type {StructuralMapContext} from './structural-map-context';
import {validStructuralIndicators} from './structural-indicators';
import {escapeContextHtml as esc} from './structural-context-html';
export interface StructuralActivityWindow {source:'stock'|'options';interval:number;startMs:number;endMs:number;buy:number;sell:number;unknown:number;net:number;events:number;observedMinutes:number;expectedMinutes:number;classifiedPct:number|null;conflicts:number;gaps:number;uncertain:boolean;direction:string;receptionCoveragePct:null;temporalCoverage:'not-certified';deltaGreekFlow:null;observedConnectionMs:number;expectedMs:number;connectionCoveragePct:number;connectionWindowComplete:boolean}
export interface StructuralActivityReport {version:'structural-activity-v1';symbol:string;at:number;storageError:string|null;windows:StructuralActivityWindow[]}
const n=(x:unknown):x is number=>typeof x==='number'&&Number.isFinite(x);
const numberFormats=new Map<string,Intl.NumberFormat>();
const fmt=(x:number|null|undefined,d=2)=>{if(x==null||!n(x))return '—';const notation=Math.abs(x)>=100000?'compact':'standard',key=d+notation;let f=numberFormats.get(key);if(!f){f=new Intl.NumberFormat('es',{maximumFractionDigits:d,notation});numberFormats.set(key,f);}return f.format(x);};
export function activityForCard(s:LiveState|undefined,c:PriceStructureCard,source:'stock'|'options',symbol:string,now:number):StructuralActivityWindow|undefined {
 const r=s?.structuralActivity;if(!r||r.version!=='structural-activity-v1'||r.symbol!==symbol||s?.symbol!==symbol||!n(r.at)||r.at>now||r.at>s.publishedMs||s.publishedMs-r.at>3000||r.storageError||!Array.isArray(r.windows)||r.windows.length>10||c.side==='SIN DATOS')return;
 const matches=r.windows.filter(w=>w.source===source&&w.interval===c.interval);if(matches.length!==1)return;
 const w=matches[0];if(!w||!n(w.startMs)||w.startMs<=0||w.startMs>=w.endMs||c.lastDuration==null||w.startMs!==c.endMs!-c.lastDuration*60000||w.endMs!==c.endMs||w.endMs>r.at||[w.buy,w.sell,w.unknown,w.net,w.events,w.observedMinutes,w.expectedMinutes,w.conflicts,w.gaps].some(v=>!n(v))||[w.buy,w.sell,w.unknown,w.events,w.observedMinutes,w.conflicts,w.gaps].some(v=>v<0)||w.expectedMinutes<=0||w.observedMinutes>w.expectedMinutes||w.receptionCoveragePct!==null||w.temporalCoverage!=='not-certified'||w.deltaGreekFlow!==null||typeof w.uncertain!=='boolean')return;
 if(!n(w.observedConnectionMs)||!n(w.expectedMs)||!n(w.connectionCoveragePct)||w.expectedMs!==w.expectedMinutes*60000||w.observedConnectionMs<0||w.observedConnectionMs>w.expectedMs||Math.abs(w.connectionCoveragePct-100*w.observedConnectionMs/w.expectedMs)>1e-7||w.connectionWindowComplete!==(w.observedConnectionMs===w.expectedMs))return;
 if(Math.abs(w.net-w.buy+w.sell)>Math.max(1e-8,(w.buy+w.sell)*1e-10))return;
 const total=w.buy+w.sell+w.unknown,pct=total>0?100*(w.buy+w.sell)/total:null;
 if(pct===null?w.classifiedPct!==null:!n(w.classifiedPct)||Math.abs(pct-w.classifiedPct)>1e-7)return;
 const direction=!total||w.uncertain||w.conflicts?'PENDIENTE':w.net>w.unknown?'COMPRADORA OBSERVADA':-w.net>w.unknown?'VENDEDORA OBSERVADA':'SIN PREDOMINIO';
 return w.direction===direction?w:undefined;
}
export function supportLabel(c:PriceStructureCard):string {
 const i=c.indicators;if(c.side==='SIN DATOS'||!validStructuralIndicators(i,c.endMs,c.bars)||!i!.ready)return 'Tendencia pendiente';
 const label=c.side==='MIXTA'?'Precio sin acuerdo':i!.trend===c.side?'Tendencia coincide':i!.trend==='MIXTA'?'Tendencia mixta':'Tendencia discrepa';
 return label+' · ADX '+fmt(i!.adx,1);
}
export function supportCardLabel(c:PriceStructureCard):string {
 const i=c.indicators;if(c.side==='SIN DATOS'||!validStructuralIndicators(i,c.endMs,c.bars)||!i!.ready)return 'Tendencia pendiente';
 return 'Tendencia '+i!.trend+' · ADX'+fmt(i!.adx,0);
}
export function supportBrief(c:PriceStructureCard):string {
 const i=c.indicators;if(c.side==='SIN DATOS'||!validStructuralIndicators(i,c.endMs,c.bars))return 'Volumen pendiente';
 return i!.rvol.value==null?'Volumen: faltan datos':'Volumen '+fmt(i!.rvol.value)+'× media';
}
export interface GammaLevel {strike:number;net:number;distanceAtr:number;distanceUsd:number}
export function gammaForCard(c:PriceStructureCard,context:StructuralMapContext):{above:GammaLevel|null;below:GammaLevel|null;positive:GammaLevel|null;negative:GammaLevel|null}|null {
 const p=context.options.profile;if(!p||c.side==='SIN DATOS'||!n(c.atr)||c.atr<=0||!n(c.lastClose)||context.options.status==='rejected')return null;
 let above:GammaLevel|null=null,below:GammaLevel|null=null,positive:GammaLevel|null=null,negative:GammaLevel|null=null;
 for(const r of p.rows){const level:GammaLevel={strike:r.strike,net:r.netGex,distanceUsd:r.strike-c.lastClose,distanceAtr:(r.strike-c.lastClose)/c.atr};
  if(level.net!==0&&level.distanceUsd>=0&&(!above||level.distanceUsd<above.distanceUsd))above=level;
  if(level.net!==0&&level.distanceUsd<=0&&(!below||level.distanceUsd>below.distanceUsd))below=level;
  const greater=(old:GammaLevel|null)=>!old||Math.abs(level.net)>Math.abs(old.net)||Math.abs(level.net)===Math.abs(old.net)&&Math.abs(level.distanceUsd)<Math.abs(old.distanceUsd);
  if(level.net>0&&greater(positive))positive=level;if(level.net<0&&greater(negative))negative=level;
 }
 return {above,below,positive,negative};
}
const comparisonCache=new WeakMap<NonNullable<StructuralMapContext['gexWindows']>,string>();
function gexComparisonTable(context:StructuralMapContext):string {
 const key=context.gexWindows;if(!key?.length)return '';const previous=comparisonCache.get(key);if(previous!==undefined)return previous;
 const html=context.gexWindows?.length?`<div class="structural-gex-windows"><b>GEX Massive · comparación de plazos acumulados</b><table><thead><tr><th>Plazo</th><th>Neto USD/1%</th><th>Mayor + / − · strike</th><th>Cobertura</th></tr></thead><tbody>${context.gexWindows.map(v=>{
  let positive:typeof v.rows[number]|undefined,negative:typeof v.rows[number]|undefined;
  for(const r of v.rows){if(r.netGex>0&&(!positive||r.netGex>positive.netGex))positive=r;if(r.netGex<0&&(!negative||r.netGex<negative.netGex))negative=r;}
  return `<tr><th title="${esc((v.start??'')+' → '+(v.end??'')+' · '+(v.expirations?.length??0)+' vencimientos')}">${esc(v.label??v.mode)}</th><td>${v.reason?'—':fmt(v.net)}</td><td>${positive?'$'+fmt(positive.strike):'—'} / ${negative?'$'+fmt(negative.strike):'—'}</td><td>${esc(v.reason||(v.complete?'Recibida':'Parcial')+' · '+v.gammaUsed+'/'+v.eligible+(v.historical?' · histórica':''))}</td></tr>`;
 }).join('')}</tbody></table><p>Los plazos comparten contratos; no se suman ni se cuentan como votos. La gamma neta positiva/negativa describe exposición convencional CALL menos PUT, no dirección alcista/bajista. Las concentraciones y distancias del perfil seleccionado aportan contexto a esta estructura de precio. La ausencia de contratos 0 DTE no se reemplaza por otra fecha.</p></div>`:'';
 comparisonCache.set(key,html);return html;
}
export function structuralSupportDetail(c:PriceStructureCard,s:LiveState|undefined,context:StructuralMapContext,now:number):string {
 const i=validStructuralIndicators(c.indicators,c.endMs,c.bars)?c.indicators:undefined;
 const indicators=!i?'<p>Indicadores pendientes: esta captura conserva la estructura anterior y no contiene estos cálculos nuevos.</p>':`<p><b>${esc(supportLabel(c))}.</b> EMA21 $${fmt(i.ema21)}; cierre menos EMA21 ${fmt(i.distanceAtr,4)} ATR; pendiente de 3 velas ${fmt(i.slope3Atr,4)} ATR. +DI ${fmt(i.plusDi)}; −DI ${fmt(i.minusDi)}; ADX14 ${fmt(i.adx)} (${esc(i.strength.toLowerCase())}). EMA9 $${fmt(i.ema9)} y EMA20 $${fmt(i.ema20)} se conservan como referencias.</p><p>Volumen relativo: ${i.rvol.value==null?'pendiente':fmt(i.rvol.value)+'×'}; ${i.rvol.references}/20 referencias. ${esc(i.rvol.method)}. Media de referencia ${fmt(i.rvol.mean)}. No compara media sesión con una sesión completa. ${esc(i.reason)} Sensibilidad EMA21 ${fmt(i.emaSeedAtr,6)} ATR y ADX ${fmt(i.adxSeedPoints,6)} puntos al retirar 14 velas.</p>`;
 const g=gammaForCard(c,context),level=(x:GammaLevel|null)=>x?'$'+fmt(x.strike)+' · '+fmt(x.net)+' USD/1% · '+fmt(x.distanceAtr,3)+' ATR desde este cierre':'sin nivel recibido';
 const gamma=g?`<p><b>GEX Massive · ${esc(context.options.profile?.label??'')} ${esc(context.options.expiry??'')}:</b> nivel recibido cercano por arriba ${esc(level(g.above))}; por abajo ${esc(level(g.below))}. Mayor concentración positiva ${esc(level(g.positive))}; negativa ${esc(level(g.negative))}. ${context.options.status==='partial'?'Perfil parcial: un strike ausente puede cambiar estas referencias.':''} Distancias calculadas contra el cierre y ATR de esta tarjeta; no contra un trade simultáneo a gamma.</p>`:'<p>GEX: sin población Massive asociada verificable.</p>';
 const comparisons=gexComparisonTable(context);
 const activity=(['stock','options'] as const).map(source=>{
  const w=activityForCard(s,c,source,context.symbol,now),label=source==='stock'?'CVD de acciones':'Flujo de opciones';
  return w&&w.events?`<p><b>${label} · misma vela cerrada · muestra parcial:</b> ${esc(w.direction.toLowerCase())}; neto ${fmt(w.net)} ${source==='stock'?'acciones':'contratos clasificados por exposición alcista/bajista inferida'}. Componentes ${fmt(w.buy)} / ${fmt(w.sell)} / ${fmt(w.unknown)} sin clasificar; clasificación ${fmt(w.classifiedPct,1)}%. Conexión del canal observada en ${fmt(w.connectionCoveragePct,2)}% de la ventana (${w.connectionWindowComplete?'muestreo completo':'muestreo parcial'}); no es cobertura de ticks. ${w.observedMinutes}/${w.expectedMinutes} minutos contienen operaciones observadas; esto no mide minutos conectados ni recepción total. ${w.gaps} reconexiones/huecos del registro; ${w.conflicts} minutos solapados excluidos; ${w.uncertain?'correcciones o identidad sin resolver':'sin incidencias declaradas en los incrementos recibidos'}. No certifica presión de todo el mercado.</p>`:`<p><b>${label}:</b> pendiente de incrementos verificables de esta temporalidad. No se sustituye por la lectura de 5 minutos.</p>`;
 }).join('');
 return `<div data-structural-support="${c.interval}">${indicators}${gamma}${comparisons}${activity}</div>`;
}
export function structuralGroups(cards:PriceStructureCard[]):{name:string;side:string;detail:string}[]{
 return [{name:'CORTO',scales:[15,60]},{name:'MEDIO',scales:[240,1440]},{name:'MAYOR',scales:[10080]}].map(g=>{
  const cs=g.scales.map(t=>cards.find(c=>c.interval===t));const valid=cs.every(c=>c&&c.side!=='SIN DATOS');
  const side=!valid?'PENDIENTE':cs.every(c=>c!.side==='CALL')?'CALL':cs.every(c=>c!.side==='PUT')?'PUT':'MIXTA';
  const trend=cs.every(c=>c&&validStructuralIndicators(c.indicators,c.endMs,c.bars)&&c.indicators!.ready);
  const agrees=trend&&cs.every(c=>c!.indicators!.trend===side);
  return {name:g.name,side,detail:!valid?'Faltan cierres verificables':!trend?'Tendencia pendiente':agrees?'Precio y tendencia coinciden':'Precio y tendencia presentan diferencias'};
 });
}
export function supportMethod():string{return `<details data-horizon-details="structural-support-method"><summary>Cómo leer los apoyos nuevos</summary><p>La estructura describe máximos y mínimos. La tendencia coincide con CALL cuando cierre &gt; EMA21, EMA21 &gt; EMA21 de 3 velas atrás y +DI &gt; −DI; para PUT se invierten las tres desigualdades. De lo contrario es mixta. Es una regla propia descriptiva, sin probabilidad calibrada. ADX mide fuerza, no dirección: referencias habituales 20 y 25, sin garantía de continuación. EMA usa α=2/(n+1), semilla SMA; DMI/ADX14 usan suavizado de Wilder, DX=100×|+DI−−DI|/(+DI+−DI), cero cuando el denominador es cero. Se exigen 100 velas y sensibilidad al retirar 14 velas ≤0,01 ATR en EMA21 y ≤0,5 puntos en ADX: controles propios.</p><p>RVOL = volumen cerrado / media de 20 barras previas comparables; intradía exige misma hora ET y duración, diario/semanal igual duración bursátil. Es actividad relativa, sin dirección. ATR14 de estas tarjetas es Wilder; el ATR diario superior conserva su método y su propósito originales.</p><p>GEX = Σ(gamma×OI×multiplicador×S²×0,01) CALL − PUT, en USD por 1% de movimiento; DEX = Σ(delta×OI×multiplicador), en acciones equivalentes. Son convenciones de exposición sobre la cadena recibida. Un GEX negativo no es una señal PUT. Delta CALL/PUT/neto pertenece a posiciones abiertas observadas, no a compras ejecutadas. Las mismas expiraciones compartidas por WEEK/NEXT4 no se suman.</p><p>Actividad: CVD suma compras menos ventas estimadas en acciones. En opciones suma contratos CALL al ask y PUT al bid menos PUT al ask y CALL al bid; solo categorías simples y cotización anterior vigente. El resto permanece desconocido. Las primas y delta por operación no se inventan: el flujo delta griego queda pendiente porque no tenemos delta causal en cada ejecución. La conexión se muestrea en la publicación existente: fase streaming y recepción de canal de hasta 3 segundos; una separación mayor de 2 segundos no se rellena. El porcentaje de tiempo corresponde a esos intervalos observados, no al censo del activo. Los registros persisten por minuto y por segmento, no se reconstruyen con velas ni OI. Los solapamientos se excluyen; incidencias de identidad/corrección retiran el predominio. Cobertura de clasificación no es recepción del mercado.</p><p>El resumen agrupa 15m/1h, 4h/diario y semanal. Conserva apoyos, diferencias y pendientes separados. Los indicadores usan los mismos precios y están relacionados; no son votos independientes. Validar fórmulas no demuestra ventaja predictiva ni asegura una dirección futura.</p></details>`;}
