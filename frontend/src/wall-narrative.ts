import type {ControlReading} from './intraday-control';
import {escapeHtml as esc} from './model';
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v);
const names=['PUT WALL','IMÁN PRINCIPAL','CALL WALL'];
export function wallNarrative(r:ControlReading,now=Date.now()) {
 const report=r.levelDynamics;
 const valid=!!report&&report.available&&report.symbol===r.symbol&&report.at<=now&&now-report.at<=3000;
 const spot=r.priceNow?.spot;
 const priceReady=r.priceNow?.ready&&finite(spot)&&spot>0;
 const direction=priceReady?r.priceNow?.direction??0:0;
 const flow=r.flowControl;
 const flowReady=flow?.quality==='ready'&&!flow.headline.includes('PARCIAL');
 const flowDirection=flowReady?(flow.tone==='buyers'?1:flow.tone==='sellers'?-1:0):0;
 const aligned=!!direction&&direction===flowDirection;
 const participation=r.participation;
 const rvol=participation&&participation.at<=now&&now-participation.at<=180000&&finite(participation.rvol)&&participation.rvol>=0?participation.rvol:undefined;
 const levels=valid?report.levels.filter(l=>names.includes(l.name)&&finite(l.strike)&&l.strike>0).map(l=>{
  const delta=priceReady?l.strike-spot!:undefined;
  const event=priceReady?r.intradayMap?.events.find(e=>e.name===l.name&&e.level===l.strike&&e.at<=now&&now-e.at<=600000):undefined;
  const change=finite(l.change)?l.change:undefined;
  const state=change==null?'Cambio aún no comparable':change>=5?'↑ FORTALECIÉNDOSE':change<=-5?'↓ DEBILITÁNDOSE':'→ ESTABLE';
  const role=delta==null?'Función pendiente de precio':Math.abs(delta)<=spot!*.0003?'PRECIO EN EL NIVEL':l.name==='IMÁN PRINCIPAL'?'REFERENCIA DE ROTACIÓN':delta<0?'SOPORTE POTENCIAL':'FRENO POTENCIAL';
  const reaction=event?`${event.action}${event.sustained?' · persistente':' · sin persistencia suficiente'}`:'Sin defensa o ruptura observada';
  const distance=delta==null?'Distancia pendiente':`${delta>=0?'+':''}${delta.toFixed(2)} USD · ${(Math.abs(delta)/spot!*100).toFixed(2)}%`;
  return {...l,state,role,reaction,distance,event};
 }):[];
 const comparable=names.every(name=>levels.filter(l=>l.name===name).length===1)&&levels.every(l=>!l.provisional&&finite(l.strength)&&finite(l.pull)&&l.flowComplete);
 const preferred=comparable?levels.find(l=>l.name===report?.preferred):undefined;
 const ahead=preferred&&priceReady&&(preferred.strike-spot!)*direction>0;
 const supported=levels.filter(l=>l.event?.sustained&&l.event.direction===direction&&!l.provisional&&l.flowComplete&&finite(l.strength));
 const reaction=supported.sort((a,b)=>b.event!.at-a.event!.at)[0];
 let headline='INFLUENCIA DE LOS NIVELES SIN CONFIRMAR';
 let reading=priceReady?`Precio $${spot!.toFixed(2)}. Las concentraciones por sí solas no confirman soporte, techo ni destino.`:'Esperando precio y velas vigentes para interpretar las concentraciones.';
 if(aligned&&reaction){
  headline=`${reaction.event!.action.toUpperCase()} · ${reaction.name}`;
  reading=`$${reaction.strike.toFixed(2)}: precio y flujo ${direction>0?'comprador':'vendedor'} acompañan la reacción. ${rvol!=null&&rvol>=1?'RVOL por encima o igual a su referencia; continuación condicionada a conservar el nivel.':'Falta participación relativa suficiente para reforzar una continuación.'}`;
 } else if(aligned&&ahead){
  headline=`RECORRIDO CON APOYO HACIA ${preferred.name}`;
  reading=`Precio $${spot!.toFixed(2)} hacia $${preferred.strike.toFixed(2)}: mayor pull comparado y flujo alineado. Es una referencia condicionada; no garantiza llegada.`;
 } else if(levels.length){
  const changing=levels.filter(l=>finite(l.change)&&Math.abs(l.change)>=5).sort((a,b)=>Math.abs(b.change!)-Math.abs(a.change!))[0];
  reading=changing?`${changing.name} $${changing.strike.toFixed(2)} ${changing.change!>0?'gana':'pierde'} fuerza (${changing.change!>0?'+':''}${changing.change!.toFixed(1)} puntos). Falta confluencia para atribuirle control del precio.`:reading;
 }
 const confirmation=`${flowReady?(aligned?'Precio y flujo alineados':'Precio y flujo sin confluencia'):'Flujo sin cobertura suficiente'} · ${rvol==null?'RVOL pendiente':`RVOL ${rvol.toFixed(2)}× a la misma hora`}`;
 return {headline,reading,confirmation,levels,preferred:preferred?.name};
}
export function wallNarrativePanel(r:ControlReading,now=Date.now()):string {
 const n=wallNarrative(r,now);
 return `<div class="wall-narrative"><strong>${esc(n.headline)}</strong><p>${esc(n.reading)}</p><small>${esc(n.confirmation)}</small></div>`;
}
