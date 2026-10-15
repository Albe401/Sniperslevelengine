import type {BlockPressure} from './overnight-block-engine';
import {etDate,etHour,etMinute} from './overnight-types';
export type Reaction='CONFIRMA'|'ABSORCIÓN POSIBLE'|'INDEFINIDA';
export interface ReactionRead {state:Reaction;changePct:number|null;reason:string}
/** Premarket price versus the notional-weighted postclose block price; descriptive, not causal impact. */
export function readReaction(blocks:BlockPressure,market?:{now:number;price:number|null;priceMs:number|null}):ReactionRead{
 if(!market||blocks.weightedPrice==null||blocks.dominance==='PENDING'||blocks.dominance==='NEUTRAL'||market.price==null||market.priceMs==null||market.price<=0||market.priceMs>market.now||market.now-market.priceMs>15000||etDate(market.now)<=blocks.date||etDate(market.priceMs)!==etDate(market.now)||etHour(market.now)<4||etHour(market.now)>9||etHour(market.now)===9&&etMinute(market.now)>=30)return {state:'INDEFINIDA',changePct:null,reason:'Reacción premarket pendiente; requiere precio reciente posterior a los bloques postcierre'};
 const changePct=100*(market.price/blocks.weightedPrice-1);
 if(!Number.isFinite(changePct))return {state:'INDEFINIDA',changePct:null,reason:'Reacción de precio inválida'};
 if(Math.abs(changePct)<.1)return {state:'INDEFINIDA',changePct,reason:'Movimiento menor a 0,10% desde el precio medio de los bloques'};
 const follows=blocks.dominance==='BUY'?changePct>0:changePct<0;
 return {state:follows?'CONFIRMA':'ABSORCIÓN POSIBLE',changePct,reason:'Comparación del precio premarket con los bloques clasificados del postcierre; no atribuye causalidad'};
}
