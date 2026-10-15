import type {ControlReading} from './intraday-control';
import type {MapLevel} from './intraday-map';
import {intradaySummary} from './intraday-summary';
const fmt=(l:{name:string;level:number})=>`${l.name} $${l.level.toFixed(2)}`;
export function controlOutlook(r:ControlReading){
 const common=r.marketEvidence;
 const summary=common?{headline:common.score.call==null?'LECTURA COMÚN · PENDIENTE':common.score.state==='Lectura mixta'?'LECTURA COMÚN · MIXTA, SIN PREDOMINIO':`LECTURA COMÚN · ${common.score.side}`,tone:common.score.call==null?'waiting':common.direction>0?'buyers':common.direction<0?'sellers':'balanced',direction:common.direction,evidence:common.detail}:r.advantage??intradaySummary(r.priceNow,r.priceLevels,r.flowControl);
 const spot=r.priceNow?.ready?r.priceNow.spot:undefined;
 const map=r.intradayMap;
 const available=(common?common.levels.map(l=>({name:l.name,level:l.price,score:l.strength??undefined,provisional:l.provisional})):map?.levels??[]).filter(l=>Number.isFinite(l.level)&&l.level>0);
 const levels=available.filter((l,i,a)=>a.findIndex(x=>x.level===l.level)===i);
 const event=map?.event;
 const walls=available.filter(l=>/^(CALL|PUT) WALL$/.test(l.name)&&l.score!=null).sort((a,b)=>b.score!-a.score!);
 const best=walls[0];
 // Keep the existing model thresholds explicit; never promote a partial chain.
 const strong=walls.length>=2&&best.score!>=55&&best.score!-walls[1].score!>=5;
 const dominant=!!(map?.complete&&strong&&spot&&event?.sustained&&event.level===best.level&&event.direction===summary.direction&&summary.direction&&Math.abs(event.level-spot)/spot<=.0035);
 const clock=event?new Date(event.at).toLocaleTimeString('es',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit',hour12:false})+' ET':'';
 const level=event?fmt(event):'Sin dominancia confirmada';
 const levelDetail=event?`${event.action} · ${clock}. ${dominant?'Dominancia inferida con apoyo del flujo.':map?.complete?'La reacción no basta para atribuir dominancia.':'Defensa/rechazo observable; peso estructural provisional.'}`:levels.length?'Hay referencias importantes; falta una interacción sostenida que decida el control.':'Esperando niveles de la sesión con datos vigentes.';
 const weight=r.mapWeightChange??'Esperando actualizaciones comparables';
 const flowChange=r.flowControl?.change??'Flujo pendiente';
 const weightChanged=/ gana peso| pierde peso/.test(weight);
 const change=weightChanged?weight:flowChange;
 const changeDetail=weightChanged?'Cambio del score del mismo muro; no significa OI entrando en vivo.':weight;
 let route='Sin recorrido preferente';
 let routeDetail=summary.direction?'Faltan niveles verificables para proponer la siguiente referencia.':'Ningún lado tiene ventaja confirmada; se conservan las referencias sin elegir destino.';
 let invalidate='Defensa o ruptura con apoyo';
 let invalidateDetail='Reevaluar cuando un lado sostenga el nivel y el flujo clasificado acompañe.';
 let target:MapLevel|undefined;
 const d=summary.direction;
 if(d&&spot){
  const ahead=levels.filter(l=>(l.level-spot)*d>0).sort((a,b)=>Math.abs(a.level-spot)-Math.abs(b.level-spot));
  const behind=levels.filter(l=>(l.level-spot)*d<0).sort((a,b)=>Math.abs(a.level-spot)-Math.abs(b.level-spot));
  target=ahead[0];
  if(target){route=fmt(target);routeDetail=target.provisional?'Referencia provisional; la cadena parcial no confirma un objetivo preferente.':`Si ${d>0?'compradores':'vendedores'} conservan la ventaja.${ahead[1]?` Tras ${d>0?'superarla':'perderla'} y sostenerse, referencia ${fmt(ahead[1])}.`:''}`;}
  const anchor=event?.direction===d&&(event.level-spot)*d<0?event:behind[0];
  if(anchor){invalidate=`${d>0?'Pérdida':'Recuperación'} de $${anchor.level.toFixed(2)}`;invalidateDetail=`Si se sostiene ${d>0?'por debajo':'por encima'} con flujo contrario, retirar la ventaja ${d>0?'compradora':'vendedora'}.`;}
 }
 if(summary.tone==='waiting'&&!d){routeDetail='La evidencia aún no permite elegir un destino; datos incompletos no equivalen a equilibrio.';}
 const center=event?.level??spot;
 const selected=center==null?levels.slice(0,3):[...levels].sort((a,b)=>Math.abs(a.level-center)-Math.abs(b.level-center)).slice(0,3);
 const rows=selected.sort((a,b)=>b.level-a.level).map(l=>({name:l.name,level:l.level,state:l.level===event?.level?event.action:l.level===target?.level?'Siguiente referencia':spot==null?'Referencia mapeada':l.level>spot?'Referencia superior':'Referencia inferior',quality:l.provisional?'Provisional':'Vigente'}));
 const flowQuality=r.flowControl?.quality??(r.flowControl?.tone==='waiting'?'waiting':r.flowControl?.headline.includes('PARCIAL')?'partial':'ready');
 const quality=[r.priceNow?.ready?'Precio: vigente':'Precio: sin evidencia vigente',flowQuality==='ready'?'Flujo: clasificado · lado inferido':flowQuality==='partial'?'Flujo: muestra parcial':'Flujo: no verificable',map?.quality??'Estructura: pendiente'];
 return {marketEvidence:common,levelEvents:map?.events??[],summary,level,levelDetail,change,changeDetail,route,routeDetail,invalidate,invalidateDetail,rows,quality,dominant,weight:best?fmt(best)+(map?.complete?'':' · provisional'):'Sin pesos comparables'};
}
