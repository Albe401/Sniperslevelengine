import {swingObservation} from './swing-observation';
import {qqqConfirmation} from './qqq-confirmation';
import {sharedLevelDetail} from './shared-level-detail';
import {atrEvidence,dailyDistance} from './atr-context';
import './route-preview.css';
import {atrDistance,type AtrRead} from './intraday-atr';
import type {LiveState} from './live';
import type {ControlReading} from './intraday-control';
import {escapeHtml as esc} from './model';
import {interpretationFresh} from './confirmations';
import {absorptionReading} from './absorption-reading';
import type {BlockPeriod} from './block-pressure';
import {displayStructure,currentMapHorizon} from './reading-display';
import {rangeFresh} from './daily-range';
import {readRouteLive} from './route-live';
import {parallelScenarios,routeSelection} from './route-parallel';
import {currentFourHour,currentFourHourReview} from './route-4h-live';
const money=(n:unknown)=>typeof n==='number'&&Number.isFinite(n)&&n>=0?new Intl.NumberFormat('en-US',{style:'currency',currency:'USD',maximumFractionDigits:0}).format(n):'—';
const delta=(n:number|null)=>n==null?'—':`${n>0?'+':''}${n.toFixed(2)}`;
const level=(n:unknown)=>typeof n==='number'&&Number.isFinite(n)&&n>0?`$${n.toFixed(2)}`:'—';
/** Snapshot sensitivity is context, never execution delta or new capital. */
export function routeData(s:LiveState|undefined,r:ControlReading,active:boolean,now=Date.now(),atr?:AtrRead) {
 const own=active&&s?.symbol===r.symbol&&interpretationFresh(s.publishedMs,now);
 const today=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York',year:'numeric',month:'2-digit',day:'2-digit'}).format(now);
 const t=s?.options.lastEventMs;
 const fresh=own&&s?.options.phase==='streaming'&&t!=null&&t<=now&&now-t<=15000;
 const w=fresh?s?.flow.intraday_windows?.find(w=>w.minutes===5):undefined;
 const hasTrades=!!w&&Number.isInteger(w.event_count)&&w.event_count>0;
 const bucket=(name:string)=>hasTrades?money(w?.buckets?.[name]?.premium):'—';
 const dynamics=own&&s?.levelDynamics?.symbol===r.symbol&&s.levelDynamics.available&&s.levelDynamics.at<=now&&now-s.levelDynamics.at<=3000?s.levelDynamics:undefined;
 const rows=(own?r.strikeReading?.rows??[]:[]).map(row=>{
  const zone=fresh&&row.expiration===today?dynamics?.levels.find(l=>l.name===row.name&&l.strike===row.strike):undefined;
  return {...row,zone:zone?`Zona $${zone.low.toFixed(2)}–$${zone.high.toFixed(2)} · prima ${money(zone.premium)} · CALL BUY ${money(zone.callBuy)} · PUT BUY ${money(zone.putBuy)} · sin clasificar ${money(zone.unknown)}${zone.flowComplete?'':' · parcial'}`:'Prima por zona pendiente'};
 });
 const iv=own?r.iv:undefined,volume=own?r.participation:undefined;
 const context=`${iv?`IV ATM ${iv.percent.toFixed(2)}% · ${iv.trend}`:'IV pendiente'} · ${volume&&volume.at<=now&&now-volume.at<=180000?`RVOL ${volume.rvol.toFixed(2)}×`:'RVOL pendiente'}`;
 const blocks=own?s?.offExchange?.analytics?.blocks:undefined;
 const blockText=(label:string,b:BlockPeriod|undefined)=>{
  if(blocks?.symbol!==r.symbol||blocks.date!==today||!b||b.count<=0||b.through_ms==null||b.through_ms>now)return `${label}: sin evidencia del día`;
  return `${label}: compras ${money(b.buy)} · ventas ${money(b.sell)} · sin clasificar ${money(b.unknown)} · ${b.count} operaciones${b.complete?'':' · historial parcial'}`;
 };
 const blockContext=blockText('Premarket',blocks?.premarket)+' | '+blockText('Sesión',blocks?.regular);
 const p=own?r.priceNow:undefined,l=own?r.priceLevels:undefined;
 const priceReady=!!p?.ready&&typeof p.spot==='number'&&Number.isFinite(p.spot)&&p.spot>0;
 const snapshot=own?s?.optionSnapshotStatus:undefined,h=currentMapHorizon(snapshot,now);
 const structure=displayStructure(h,snapshot?.phase==='ready'&&rangeFresh(snapshot,now)&&!!h&&rangeFresh({phase:'ready',observedMs:h.observedMs},now));
 const routes=(structure?.routes??[]).map(route=>`${route.direction}: barrera ${level(route.barrier)} · pull ${level(route.pullTarget)} · vacío ${level(route.vacuumLow)}–${level(route.vacuumHigh)}`).join(' | ');
 const absorption=absorptionReading(own?r:{...r,priceNow:undefined,flowControl:undefined,participation:undefined},now);
 const experiment=readRouteLive(s,r.symbol,!!own&&priceReady,now,atr);
 const direction=priceReady?experiment.result.direction:0;
 const referenceDirection=direction||p?.direction||0;
 const spot=priceReady?p!.spot!:null;
 const next=['reached','invalidated'].includes(experiment.directional.state)?null:experiment.result.target??(spot!=null&&referenceDirection!==0?experiment.structure.levels.filter(l=>(l.price-spot)*referenceDirection>spot*.0003).sort((a,b)=>Math.abs(a.price-spot)-Math.abs(b.price-spot))[0]?.price:null)??null;
 const decisive=experiment.result.level??(spot!=null?experiment.structure.levels.filter(l=>l.role!=='vacuum-edge'&&l.role!=='pull').sort((a,b)=>Math.abs(a.price-spot)-Math.abs(b.price-spot))[0]?.price:null)??null;
 const basket=qqqConfirmation(s,r.symbol,!!own,direction>0?'CALL':direction<0?'PUT':'PENDIENTE',now);
 const stage=experiment.assessment.label;
 const heading=experiment.result.state==='invalidated'?'Ruta invalidada':experiment.result.state==='reached'?'Referencia alcanzada':direction>0?'Escenario hacia arriba ↑':direction<0?'Escenario hacia abajo ↓':'Sin activación de precio';
 return {components:basket,vanna:own&&r.vannaEngine?.symbol===r.symbol&&r.vannaEngine.at===now?r.vannaEngine.levels.find(l=>l.strike===decisive&&l.expiration===today)?.support:undefined,symbol:r.symbol,atrNext:priceReady&&spot!=null&&next!=null?[atrDistance(atr,spot,next),dailyDistance(atr,spot,next)].filter(Boolean).join(' · '):'',flow:`CALL BUY ${bucket('CALL BUY')} · PUT BUY ${bucket('PUT BUY')}`,
  flowDetail:hasTrades?`${w!.event_count} ejecuciones · ${w!.contracts} contratos · sin clasificar ${bucket('UNKNOWN')}${w!.premium_complete?'':' · prima parcial'}`:'Sin operaciones recientes recibidas',
  rows,context:context+` · ${atr?.value!=null?`ATR14 5 min $${atr.value.toFixed(2)}${next!=null&&spot!=null?` · próxima referencia a ${atrDistance(atr,spot,next)}`:''}`:'ATR intradía pendiente'}`,blockContext,price:priceReady?`$${p!.spot!.toFixed(2)} · ${p!.headline}`:'Esperando precio vigente',
  priceDetail:priceReady?[p!.structure,p!.vwap,p!.change].filter(Boolean).join(' · '):'No se utiliza un precio antiguo para emitir señales',
  levels:[l?.important??'Esperando niveles vigentes',routes].filter(Boolean).join(' | '),reaction:experiment.result.level!=null?experiment.reason:l?.reaction??'Esperando prueba del nivel',
  target:priceReady?experiment.target??l?.route??'Referencia pendiente':'Esperando precio vigente',
  condition:priceReady?experiment.result.level!=null?experiment.reason:l?.condition??'Esperando condición':'Esperando evidencia actual',
  absorption:absorption.headline,absorptionDetail:absorption.detail,invalidation:experiment.invalidation??(absorption.kind==='waiting'?'Pendiente de un recorrido confirmado':absorption.invalidation),
  directional:experiment.directional,conclusion:experiment.conclusion,experiment,compact:{spot:level(spot),direction:heading,decisive:level(decisive),next:level(next),stage,stop:level(experiment.result.stop),stopSide:experiment.result.level!=null&&experiment.result.stop!=null&&experiment.result.stop>experiment.result.level?'Cierre sobre':'Cierre bajo'},quality:'PRUEBA EN VIVO · SIN EJECUCIÓN'};
}
export function routePreview(s?:LiveState,r?:ControlReading,active=false,now=Date.now(),atr?:AtrRead):string {
 const d=routeData(s,r??{symbol:s?.symbol??'—'} as ControlReading,active,now,atr);
 const projection=d.directional;
 const pathLabel=projection.state==='waiting'?d.compact.direction:projection.label;
 const distance=projection.distance==null?'':`Recorrido $${projection.distance.toFixed(2)}${projection.atrDaily==null?'':` · ${projection.atrDaily.toFixed(2)} ATR D`}`;
 const swing=swingObservation(s,d.symbol,active,now);
 const text=(value:string)=>`<strong>${esc(value)}</strong>`;
 const detail=(value:string)=>`<small>${esc(value)}</small>`;
 const more=(id:string,body:string)=>`<details class="route-preview-more" data-horizon-details="route-${id}"><summary>Ver datos</summary>${body}</details>`;
 const station=(n:string,title:string,body:string)=>`<article class="route-preview-station"><span class="route-preview-number">${n}</span><h3>${title}</h3><div class="route-preview-data">${body}</div></article>`;
 const rows=d.rows.map(row=>`<div class="route-strike-row"><b>${esc(row.name)} $${row.strike.toFixed(2)}</b><small>Δ C ${delta(row.call)} / P ${delta(row.put)} · ${esc(row.expiration)}</small><small>${row.strength==null?'Fuerza pendiente':`Fuerza ${row.strength.toFixed(1)}/100`}${row.change==null?'':` · cambio ${row.change>0?'+':''}${row.change.toFixed(1)}`} · ${esc(row.pressure)}</small><small>${esc(row.zone)}</small></div>`).join('');
 const structural=d.experiment.structure;
 const levelDetails=detail(structural.reason)+structural.levels.map(l=>detail(`${l.name} $${l.price.toFixed(2)} · ${l.role==='vacuum-edge'?'intensidad':'fuerza'} ${l.strength==null?'pendiente':l.strength.toFixed(1)+'/100 · '+l.grade}${l.change==null?'':' · cambio '+l.change.toFixed(1)}${l.provisional?' · provisional':''}`)+sharedLevelDetail(d.experiment.market,l.price)).join('');
 const selection=routeSelection(d.symbol),alternatives=parallelScenarios(d.experiment.market,d.experiment.result,now);
 const scenarioCards=alternatives.map(a=>station(a.side==='CALL'?'↑':'↓',`Escenario ${a.side}`,
  `<small class="route-parallel-state" title="${esc(a.state)}">${esc(a.state)}${a.provisional?' · provisional':''}</small>`+
  text(a.trigger==null?'Activación pendiente':`${a.direction===1?'Superar':'Perder'} ${level(a.trigger)}${a.actual?' · prueba observada':''}`)+
  detail(`Referencias: ${level(a.target)}${a.next==null?'':' → '+level(a.next)}`)+
  detail(a.stop==null?'Invalidación: pendiente':`Invalida: cierre ${a.direction===1?'bajo':'sobre'} ${level(a.stop)}`)+
  `<button type="button" data-route-side="${a.side}" data-route-symbol="${esc(d.symbol)}" aria-pressed="${selection.side===a.side}">Ver proyección ${a.side}</button>`)).join('');
 const review=active&&s?.symbol===d.symbol?currentFourHourReview(d.symbol,now):undefined;
 const four=review?currentFourHour(d.symbol,now):undefined,scenario=four?.scenario;
 const matching=scenario&&(scenario.side===1?'CALL':'PUT')===selection.side;
 const historical=matching?review?.projections?.find(p=>p.horizon===selection.horizon):undefined;
 const valid=historical?.state==='descriptive'&&historical.probabilityClaim===false&&Number.isInteger(historical.n)&&historical.n>=20&&[historical.low,historical.typical,historical.high].every(v=>typeof v==='number'&&Number.isFinite(v)&&v>0)&&((historical.typical!-scenario!.origin)*scenario!.side)>0&&((historical.typical!-historical.low!)*scenario!.side)>=0&&((historical.high!-historical.typical!)*scenario!.side)>=0;
 const projectionBody=`<div class="route-projection-controls">${(['session','swing2'] as const).map(h=>`<button type="button" data-route-horizon="${h}" data-route-symbol="${esc(d.symbol)}" aria-pressed="${selection.horizon===h}">${h==='session'?'Intradía':'Swing · 2 sesiones'}</button>`).join('')}</div>`+
  text(valid?`Mediana ${level(historical!.typical)}`:historical?'Historial insuficiente':'Proyección pendiente')+
  detail(valid?`Zona ${level(Math.min(historical!.low!,historical!.high!))}–${level(Math.max(historical!.low!,historical!.high!))}`:`${selection.side} · ${matching?'Sin recorrido estadístico suficiente':'Esperando escenario 4H vigente'}`)+
  detail(historical?`${historical.n} casos comparables${valid?' · descriptivo':' / mínimo 20'}`:'Sin datos para proyectar');
 return `<section class="route-preview route-parallel" aria-label="Mapa de recorrido">
 <header><div><span>MAPA DE RECORRIDO · ${esc(d.symbol)} · PRECIO ${esc(d.compact.spot)}</span><h2>Ruta del precio</h2></div><small>${d.quality} · ${esc(swing.label)}</small></header>
 ${quantRoutePanel(d.experiment.quant)}
 <div class="route-intraday-signal"><strong>${esc(d.experiment.intraday.label)}</strong><small>${esc(d.experiment.intraday.reason)}</small><small>Referencia intradía ${level(d.experiment.intraday.target)} · invalida ${level(d.experiment.intraday.stop)} · precio + CVD · experimental</small></div>
 <div class="route-preview-journey">
 ${scenarioCards}${station('◇','Proyección histórica',projectionBody)}
 </div>
 <footer><span class="route-preview-number">5</span><div class="route-preview-conclusion"><h3>${esc(pathLabel)}</h3>${detail(d.compact.stage)}</div><div><span>APOYO CVD</span><small class="route-cvd-status">${esc(d.experiment.cvd.label+(d.components?' · '+d.components.label:''))}</small></div><div><span>SIGUIENTE REFERENCIA</span>${detail(d.compact.next)}</div><div><span>INVALIDACIÓN</span>${detail(d.compact.stop==='—'?'Pendiente':d.compact.stopSide+' '+d.compact.stop)}</div></footer>
 ${more('experiment',detail('Escenarios condicionales del mapa actual; superar/perder un nivel exige confirmación de precio, prueba y continuación. Son alternativas, no dos señales simultáneas. Invalidaciones solo del escenario realmente observado.')+detail('Precio actual: '+d.compact.spot)+detail('Camino del precio: '+pathLabel)+detail('Nivel decisivo: '+d.compact.decisive)+detail('Siguiente referencia: '+d.compact.next)+detail(distance)+detail(d.atrNext)+detail(d.flow)+detail(d.flowDetail)+rows+detail(d.blockContext)+detail(d.price)+detail(d.context)+detail(d.absorption)+detail(d.priceDetail)+detail(d.absorptionDetail)+detail(d.reaction)+detail(d.levels)+detail(d.condition)+detail(d.target)+detail('Motor direccional · '+projection.label)+detail(projection.detail)+detail(projection.invalidation)+detail(four?.text??'Contexto 4H: esperando datos vigentes')+(valid?detail(`Retroceso típico ${level(historical!.adverse)} · pico ${historical!.peakRegularMinutes?.toFixed(0)??'—'} min RTH; no es hora de llegada`):'')+detail('Zona histórica P25–P75, descriptiva; no es probabilidad calibrada. Seleccionar CALL o PUT cambia la consulta visual, no la señal del motor ni el bot.')+(projection.expectedMove==null?'':detail('Movimiento esperado del snapshot ±$'+projection.expectedMove.toFixed(2)+' · magnitud sin dirección'))+swing.html+atrEvidence(atr)+detail(d.conclusion)+detail(d.experiment.assessment.detail)+(d.components?detail(d.components.detail):'')+detail(d.experiment.market.detail)+detail(d.experiment.reason)+levelDetails+detail(d.experiment.note)+detail(d.experiment.storage)+'<button type="button" data-route-export>Exportar prueba</button>')}
 </section>`;
}





import {quantRoutePanel} from './quant-route-live';
