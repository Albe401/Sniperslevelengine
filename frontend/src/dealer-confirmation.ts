import type {ControlReading} from './intraday-control';
import {deriveView} from '../../integrations/neuro-trader-engine/runtime/frontend/src/lib/analytics';
import {escapeHtml as esc} from './model';
export function dealerConfirmation(r:ControlReading) {
 const p=r.priceNow,f=r.flowControl,one=r.components.find(c=>c.name==='FLOW 1M')?.signal,five=r.components.find(c=>c.name==='FLOW 5M')?.signal;
 const side=(n:number|undefined)=>n==null?0:n>=.15?1:n<=-.15?-1:0;
 const price=p?.ready?(p.direction??0):0,flow=f?.quality==='ready'?(f.tone==='buyers'?1:f.tone==='sellers'?-1:0):0;
 const windows=side(one)!==0&&side(one)===side(five);
 let theory=0;
 const c=r.neuroContext;
 if(c?.available&&c.symbol===r.symbol&&c.modelPayload&&c.at<=Date.now()&&Date.now()-c.at<=45000){try{const b=deriveView(c.modelPayload,true,'std').sc.bias;theory=b>=60?1:b<=40?-1:0;}catch{}}
 const dispute=!!(price&&flow&&price!==flow||flow&&theory&&flow!==theory||side(one)&&side(five)&&side(one)!==side(five));
 const confirmed=!dispute&&windows&&price===flow&&flow!==0&&r.advantage?.direction===flow;
 const headline=dispute?'LECTURA EN DISPUTA':confirmed?`DIRECCIÓN ${flow>0?'ALCISTA':'BAJISTA'} CONFIRMADA POR PRECIO Y FLUJO`:'DIRECCIÓN INTRADÍA SIN CONFIRMAR';
 const factors=[p?.ready?`Precio: ${price>0?'avance':price<0?'retroceso':'sin dirección'} · ${p.headline??''}`:'Precio: falta evidencia vigente',f?.evidence??'Flujo: pendiente',windows?'Flujo 1m y 5m: coinciden':one==null||five==null?'Flujo 1m/5m: falta cobertura suficiente':'Flujo 1m/5m: mixto o contradictorio',`Estructura: ${r.priceLevels?.quality??'pendiente'}`,r.priceLevels?.reaction??'Reacción en niveles pendiente',r.iv?.context??'Volatilidad: falta comparación vigente'];
 return `<section class="dealer-confirmation"><h3>${headline}</h3><p>${confirmed?'Confirmación descriptiva con persistencia; no garantiza continuación.':dispute?'Los factores disponibles se contradicen; el contexto de dealers no resuelve la dirección.':'Falta precio y flujo suficiente, alineado y persistente en nuevas observaciones.'}</p><ul>${factors.map(x=>`<li>${esc(x)}</li>`).join('')}</ul><small>Contexto de dealers: modelo de siete días. Estructura: horizonte del mapa. Cobertura parcial limita la lectura; datos ausentes no equivalen a neutralidad. Esta lectura no modifica las reglas del bot.</small></section>`;
}
