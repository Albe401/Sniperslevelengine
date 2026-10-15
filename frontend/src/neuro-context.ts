import {escapeHtml as esc} from './model';
export interface NeuroContext {symbol:string;at:number;available:boolean;reason:string;netGamma:number|null;charmFlow:number|null;gammaMagnet:number|null;gammaFlip:number|null;contracts:number;used:number;modelPayload?:import('../../integrations/neuro-trader-engine/runtime/frontend/src/lib/types').LivePayload|null}
export function neuroText(c:NeuroContext|undefined,symbol:string,active:boolean,now=Date.now(),side=''):string {
 if(!active||!c||c.symbol!==symbol||!c.available||c.at>now||now-c.at>45000)return 'Neuro: contexto no vigente; lectura basada en la evidencia de Snipers.';
 const gamma=c.netGamma==null?'Gamma pendiente':c.netGamma>0?'Gamma positiva: posible amortiguación':c.netGamma<0?'Gamma negativa: posible amplificación':'Gamma sin predominio';
 const charm=c.charmFlow==null?'Charm pendiente':c.charmFlow>0?'charm estima presión compradora':c.charmFlow<0?'charm estima presión vendedora':'charm sin dirección';
 const conflict=(side==='CALL'&&(c.charmFlow??0)<0)||(side==='PUT'&&(c.charmFlow??0)>0);
 const magnet=c.gammaMagnet!=null&&Number.isFinite(c.gammaMagnet)&&c.gammaMagnet>0?` Concentración gamma $${c.gammaMagnet.toFixed(2)}; contrastar con el imán de Snipers.`:'';
 return `${gamma}; ${charm}.${conflict?' Divergencia con Snipers.':''}${magnet} Modelo de siete días, 0DTE incluido; proyección bajo convención +CALL/−PUT, no flujo observado. Cobertura ${c.used}/${c.contracts}; OI sin fecha certificada. No suma puntuación ni confirma una entrada.`;
}
export function neuroContextLine(c:NeuroContext|undefined,symbol:string,active:boolean,now=Date.now(),side=''):string {
 return `<p class="panel-caption neuro-context">${esc(neuroText(c,symbol,active,now,side))}</p>`;
}
