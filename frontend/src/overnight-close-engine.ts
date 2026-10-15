import type {ClosingBar,Side} from './overnight-types';
import type {BlockPressure} from './overnight-block-engine';
export type CloseLocation='MÁXIMO'|'MEDIO'|'MÍNIMO'|'PENDIENTE';
export type Scenario='CONTINUACIÓN CALL'|'CONTINUACIÓN PUT'|'PRECAUCIÓN · POSIBLE PUT'|'PRECAUCIÓN · POSIBLE CALL'|'NEUTRAL'|'PENDIENTE';
export interface CloseRead {location:CloseLocation;position:number|null;scenario:Scenario;bias:Side;reason:string}
export function readClose(blocks:BlockPressure,bar:ClosingBar|null):CloseRead{
 if(!bar||bar.symbol!==blocks.symbol||bar.date!==blocks.date||![bar.open,bar.high,bar.low,bar.close,bar.volume].every(Number.isFinite)||bar.low<=0||bar.high<Math.max(bar.open,bar.close)||bar.low>Math.min(bar.open,bar.close)||bar.high===bar.low)return {location:'PENDIENTE',position:null,scenario:'PENDIENTE',bias:'PENDING',reason:'Cierre diario completo y verificable pendiente'};
 const position=100*(bar.close-bar.low)/(bar.high-bar.low),location=position>=75?'MÁXIMO':position<=25?'MÍNIMO':'MEDIO';
 if(blocks.dominance==='PENDING')return {location,position,scenario:'PENDIENTE',bias:'PENDING',reason:blocks.reason};
 if(blocks.dominance==='NEUTRAL')return {location,position,scenario:'NEUTRAL',bias:'NEUTRAL',reason:'Compras y ventas estimadas equilibradas'};
 if(blocks.dominance==='BUY'&&location==='MÁXIMO')return {location,position,scenario:'CONTINUACIÓN CALL',bias:'BUY',reason:'Bloques compradores postcierre y cierre previo en el cuarto superior'};
 if(blocks.dominance==='SELL'&&location==='MÍNIMO')return {location,position,scenario:'CONTINUACIÓN PUT',bias:'SELL',reason:'Bloques vendedores postcierre y cierre previo en el cuarto inferior'};
 if(blocks.dominance==='BUY'&&location==='MÍNIMO')return {location,position,scenario:'PRECAUCIÓN · POSIBLE PUT',bias:'PENDING',reason:'Bloques compradores postcierre frente a un cierre previo débil; requiere confirmación premarket'};
 if(blocks.dominance==='SELL'&&location==='MÁXIMO')return {location,position,scenario:'PRECAUCIÓN · POSIBLE CALL',bias:'PENDING',reason:'Bloques vendedores postcierre frente a un cierre previo fuerte; requiere confirmación premarket'};
 return {location,position,scenario:'NEUTRAL',bias:'NEUTRAL',reason:'Cierre medio; no se confirma continuación'};
}
