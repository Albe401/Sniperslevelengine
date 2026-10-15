import type {ControlReading} from './intraday-control';
import type {LiveState} from './live';
import type {BlockPeriod} from './block-pressure';
import {absorptionReading} from './absorption-reading';
import {interpretationFresh} from './confirmations';
import {rangeFresh} from './daily-range';
import {displayStructure,currentMapHorizon} from './reading-display';
import {escapeHtml as esc} from './model';
import './bias-route-map.css';
const finite=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n);
const money=(n:number)=>new Intl.NumberFormat('en-US',{style:'currency',currency:'USD',notation:'compact',maximumFractionDigits:1}).format(n);
export function biasRouteEvidence(s:LiveState|undefined,r:ControlReading,active:boolean,now=Date.now()) {
 const own=active&&s?.symbol===r.symbol&&interpretationFresh(s.publishedMs,now);
 const at=s?.options?.lastEventMs;
 const flowFresh=own&&s?.options.phase==='streaming'&&finite(at)&&at<=now&&now-at<=15000;
 const five=flowFresh?s?.flow?.intraday_windows?.find(w=>w.minutes===5):undefined;
 const premiumObserved=!!five&&five.event_count>0&&['CALL BUY','PUT BUY','CALL SELL','PUT SELL','UNKNOWN'].every(k=>finite(five.buckets?.[k]?.premium)&&five.buckets![k].premium>=0);
 const premium=premiumObserved&&!!five?.premium_complete;
 const amount=(k:string)=>premiumObserved?money(five!.buckets![k].premium):'—';
 const blocks=own?s?.offExchange?.analytics?.blocks:undefined;
 const date=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York',year:'numeric',month:'2-digit',day:'2-digit'}).format(now);
 const blockValid=blocks?.symbol===r.symbol&&blocks.date===date;
 const blockText=(period:BlockPeriod|undefined,recent:boolean)=>{
  if(!blockValid||!period||period.count<=0||!finite(period.through_ms)||period.through_ms>now||(recent&&now-period.through_ms>15000)||![period.buy,period.sell,period.unknown].every(n=>finite(n)&&n>=0))return 'Sin evidencia vigente';
  return `Compras ${money(period.buy)} · ventas ${money(period.sell)} · cobertura ${finite(period.coverage_pct)&&period.coverage_pct>=0&&period.coverage_pct<=100?period.coverage_pct.toFixed(1)+'%':'pendiente'}${period.complete?'':' · muestra parcial'}`;
 };
 const premarket=blockText(blocks?.premarket,false),regular=blockText(blocks?.regular,true);
 const rows=own?r.strikeReading?.rows??[]:[];
 const deltaRows=rows.filter(l=>l.call!=null||l.put!=null);
 const f=own?r.flowControl:undefined,p=own?r.priceNow:undefined;
 const dir=f?.quality==='ready'&&!f.headline.includes('PARCIAL')?(f.tone==='buyers'?1:f.tone==='sellers'?-1:0):0;
 const event=own&&p?.ready?r.intradayMap?.events.find(e=>e.sustained&&e.at<=now&&now-e.at<=180000&&r.intradayMap?.levels.some(l=>l.name===e.name&&l.level===e.level)&&finite(p.spot)&&Math.abs(e.level-p.spot)/p.spot<=.0035):undefined;
 const absorption=own?absorptionReading(r,now):{kind:'waiting',headline:'SIN INDICIOS SUFICIENTES',detail:'Esperando conexión y evidencia vigente.'};
 const counter=!!p?.ready&&/RETROCESO|REBOTE/.test(p.headline);
 // Visual candidate only. Snapshot delta is never presented as execution delta,
 // and the existing two-second gamma transition never confirms this model.
 const candidate=!counter&&premium&&dir!==0&&p?.ready&&p.direction===dir&&event?.direction===dir&&absorption.kind==='waiting'?dir:0;
 const direction=absorption.kind==='fall'?1:absorption.kind==='rise'?-1:p?.ready?p.direction??0:0;
 const h=own?currentMapHorizon(s?.optionSnapshotStatus,now):undefined;
 const structure=displayStructure(h,!!h&&rangeFresh({phase:'ready',observedMs:h.observedMs},now));
 const targets:{name:string;level:number}[]=[];
 const add=(name:string,level:number|null|undefined)=>{if(finite(level)&&level>0)targets.push({name,level});};
 if(structure){add('Techo de opciones (Call Wall)',structure.callWall?.strike);add('Piso de opciones (Put Wall)',structure.putWall?.strike);add('Imán principal',structure.selected.M1);add('Imán de liquidez',structure.magnet);for(const route of structure.routes){add('Barrera de liquidez',route.barrier);add(route.direction==='CALL'?'Atracción superior (Call Pull)':'Atracción inferior (Put Pull)',route.pullTarget);}}
 const spot=p?.ready&&finite(p.spot)?p.spot:null;
 const next=spot!=null&&direction!==0?targets.filter(t=>(t.level-spot)*direction>spot*.0003).sort((a,b)=>Math.abs(a.level-spot)-Math.abs(b.level-spot))[0]:undefined;
 const slowing=!!p?.ready&&p.pace?.includes('PERDIENDO VELOCIDAD');
 return {premium,premiumObserved,premarket,regular,callBuy:amount('CALL BUY'),putBuy:amount('PUT BUY'),callSell:amount('CALL SELL'),putSell:amount('PUT SELL'),unknown:amount('UNKNOWN'),rows,deltaRows,priceReady:!!p?.ready,price:p?.headline??'Precio pendiente',flow:f?.headline??'Flujo pendiente',event,absorption,counter,candidate,comparisonReady:!!p?.ready&&dir!==0,confirmation:false as const,next,spot,direction,slowing};
}
export function biasRouteMap(s:LiveState|undefined,r:ControlReading,active:boolean,now=Date.now()):string {
 const e=biasRouteEvidence(s,r,active,now);
 const delta=(n:number|null)=>n==null?'—':`${n>0?'+':''}${n.toFixed(2)}`;
 const levels=e.rows.slice(0,5).map(l=>`${l.name} ${l.strike.toFixed(2)}`).join(' · ');
 const deltaText=e.deltaRows.slice(0,3).map(l=>`${l.strike.toFixed(2)} · C ${delta(l.call)} / P ${delta(l.put)}`).join(' · ');
 const node=(id:string,title:string,headline:string,detail:string,ready:boolean,tone='cyan')=>`<article class="bias-route-node ${tone} ${ready?'has-evidence':''}" data-bias-node="${id}"><span>${title}</span><strong>${esc(headline)}</strong><p>${esc(detail)}</p></article>`;
 const state=e.counter?'RETROCESO / REBOTE · SIN GIRO CONFIRMADO':e.absorption.kind!=='waiting'?'POSIBLE ABSORCIÓN · ESPERAR CONFIRMACIÓN':e.candidate?`CAMBIO HACIA ${e.candidate>0?'CALL':'PUT'} · EN EVALUACIÓN`:'SESGO FINAL PENDIENTE';
 const route=e.next?`${e.direction>0?'Si sostiene la recuperación':'Si sostiene la pérdida'}: próxima referencia ${e.next.name} $${e.next.level.toFixed(2)}. No garantiza alcanzarla.`:'Esperando precio vigente y un nivel por delante.';
 return `<section class="bias-route-map" aria-label="Mini mapa de transición del sesgo"><header><div><span>MINI MAPA · PRECIO, DINERO Y NIVELES</span><h3>¿Qué está pasando y hasta dónde podría ir? · ${esc(r.symbol)}</h3></div><small>↑ CALL: subida · ↓ PUT: caída</small></header><div class="bias-route-status">${esc(state)}</div><div class="bias-route-track">
 <div class="bias-route-row">${node('premium','01 · PRIMA 0DTE · ÚLTIMOS 5 MIN',e.premium?'Actividad observada':e.premiumObserved?'Actividad parcial':'Esperando operaciones',`CALL BUY ${e.callBuy} · PUT BUY ${e.putBuy}`,e.premium,'green')}${node('delta','02 · SENSIBILIDAD DE LAS OPCIONES',e.deltaRows.length?'Delta del snapshot':'Delta pendiente',deltaText||'Mismo strike y vencimiento; sin valores inventados.',e.deltaRows.length>0)}</div>
 <div class="bias-route-arrow" aria-hidden="true">↓</div>
 <div class="bias-route-row">${node('levels','03 · PISOS, TECHOS E IMANES',e.rows.length?'Referencias del mapa':'Niveles pendientes',levels||'Walls, imanes, pulls y barreras según evidencia disponible.',e.rows.length>0)}${node('reaction','04 · ¿QUÉ HACE EL PRECIO?',e.event?`${e.event.name} ${e.event.level.toFixed(2)}`:e.price,e.event?.action??'Esperando defensa, rechazo o ruptura persistente.',!!e.event,'pink')}</div>
 <div class="bias-route-arrow" aria-hidden="true">↓</div>
 ${node('compare','05 · ¿EL DINERO ACOMPAÑA EL MOVIMIENTO?',e.counter?'Retroceso / rebote':e.comparisonReady?'Comparar reacción y flujo':'Comparación pendiente',e.flow,e.comparisonReady,'yellow')}
 <div class="bias-route-arrow" aria-hidden="true">↓</div>
 <div class="bias-route-row">${node('absorption','06 · ¿DÓNDE SE ESTÁ FRENANDO?',e.absorption.headline,e.absorption.detail,e.absorption.kind!=='waiting','yellow')}${node('candidate','07 · ¿CAMBIA DE DIRECCIÓN?',e.candidate?`HACIA ${e.candidate>0?'CALL ↑':'PUT ↓'}`:'Sin candidato confirmado',e.counter?'Un retroceso no cambia el sesgo.':e.event?'Exigir prueba del nivel y persistencia en flujo nuevo.':'Esperando interacción sostenida con el nivel decisivo.',e.candidate!==0,e.candidate<0?'pink':'green')}</div>
 <div class="bias-route-arrow" aria-hidden="true">↓</div>
 <div class="bias-route-row">${node('target','08 · ¿HASTA DÓNDE PODRÍA LLEGAR?',e.next?'$'+e.next.level.toFixed(2):'Próximo nivel pendiente',route,!!e.next,e.direction<0?'pink':'green')}${node('exhaustion','09 · ¿SE AGOTA EL RECORRIDO?',e.slowing?'El avance pierde velocidad':e.absorption.kind!=='waiting'?'Hay oposición en el nivel':'Sin agotamiento verificado',e.slowing?'Puede estar frenándose; aún no confirma una vuelta.':e.absorption.kind!=='waiting'?'El precio encuentra oposición; esperar ruptura o rechazo sostenido.':'No deducir agotamiento por un solo retroceso.',e.slowing||e.absorption.kind!=='waiting','yellow')}</div>
 <div class="bias-route-arrow" aria-hidden="true">↓</div>
 ${node('confirm','10 · ¿SE CONFIRMA EL CAMBIO?','Todavía no se confirma','Falta evidencia completa del dinero negociado y su dirección a lo largo del tiempo. No anunciar un giro por un solo movimiento.',false)}
 <div class="bias-route-arrow" aria-hidden="true">↙ &nbsp; ↘</div>
 <div class="bias-route-row bias-route-final">${node('call','RESULTADO CALL ↑','PENDIENTE','Recuperación + prueba sostenida + flujo y delta compatibles.',false,'green')}${node('put','RESULTADO PUT ↓','PENDIENTE','Pérdida + prueba sostenida + flujo y delta compatibles.',false,'pink')}</div>
 </div><details data-horizon-details="bias-route-method"><summary>Datos y condiciones del recorrido</summary><p>Bloques premarket (contexto): ${esc(e.premarket)}. Bloques sesión (acumulado): ${esc(e.regular)}. No confirman por sí solos un giro ni se suman a la prima de opciones.</p><p>Prima 0DTE: CALL SELL ${e.callSell} · PUT SELL ${e.putSell} · sin clasificar ${e.unknown}. Compras/ventas inferidas frente al bid/ask; no apertura/cierre de posiciones. Las primas no son entradas netas de dinero.</p><p>Referencias y deltas: ${esc(e.rows.map(l=>`${l.name} ${l.strike.toFixed(2)} · ${l.expiration}${l.partial?' provisional':''}`).join('; ')||'Pendientes')}. Vacíos: rangos sin delta propio. Los cuadros iluminados indican evidencia disponible, no una confirmación del sesgo.</p><p>Absorción: divergencia compatible con absorción; no confirma liquidez pasiva del activo. Caducar la evidencia o cambiar activo, sesión o vencimiento retira el candidato. El modelo final requiere completar el agregador por ejecución; no usa la confirmación rápida del mapa gamma ni cambia las entradas del bot.</p></details></section>`;
}
