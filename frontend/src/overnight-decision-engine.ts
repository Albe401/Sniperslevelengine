import type {Side} from './overnight-types';
import type {BlockPressure} from './overnight-block-engine';
import type {CloseRead} from './overnight-close-engine';
import type {ReactionRead} from './overnight-reaction-engine';
import type {PremarketRead} from './overnight-premarket-engine';
import type {QqqPressure} from './overnight-qqq-engine';
export interface OvernightDecision {bias:Side;status:'PENDIENTE'|'CONFIRMADO'|'INVALIDADO'|'MIXTO';evidenceScore:number|null;reason:string}
export function assembleOvernight(blocks:BlockPressure,close:CloseRead,reaction:ReactionRead,premarket:PremarketRead,components:QqqPressure):OvernightDecision{
 const bias=close.bias;
 if(bias==='PENDING')return {bias,status:'PENDIENTE',evidenceScore:null,reason:'Bloques clasificados y cierre completo pendientes'};
 if(bias==='NEUTRAL')return {bias,status:'MIXTO',evidenceScore:null,reason:'Cierre sin ventaja direccional'};
 let score=25+Math.min(25,blocks.coverage*.25);
 if(blocks.lastHour===bias)score+=15;
 if(reaction.state==='CONFIRMA')score+=10;
 if(reaction.state==='ABSORCIÓN POSIBLE')score-=10;
 if(components.dominance===bias)score+=10;
 if(components.dominance!=='PENDING'&&components.dominance!=='NEUTRAL'&&components.dominance!==bias)score-=10;
 if(premarket.state==='CONFIRMADO')score+=15;
 if(premarket.state==='INVALIDADO')score-=20;
 return {bias,status:premarket.state,evidenceScore:Math.max(0,Math.min(100,score)),reason:'Índice de evidencia heurístico, no probabilidad. No habilita entradas automáticas.'};
}
