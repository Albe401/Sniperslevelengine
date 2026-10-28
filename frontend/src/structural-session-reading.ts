import {PRICE_STRUCTURE_RULE,type PriceStructureCard} from './price-structure';
export interface SessionDirectionEvidence {
 symbol:string;availability:'current'|'reference'|'pending'|'rejected';side:'CALL'|'PUT'|'MIXTA'|'PENDIENTE';
 closedAtMs:number|null;livePriceReady:boolean;reason:string;
 triggerConfirmed:false;probability:null;executionEnabled:false;
}

/** Presentation states only: never promote a pending engine result or authorize an entry. */
export function structureDisplayState(card:PriceStructureCard):string {
 if(card.side!=='SIN DATOS')return card.side;
 const measured=card.bars>0&&Number.isFinite(card.endMs)&&card.endMs!>0&&Number.isFinite(card.lastClose)&&card.lastClose!>0&&Number.isFinite(card.atr)&&card.atr!>0;
 if(!measured)return 'SIN DATOS';
 if(card.bars<PRICE_STRUCTURE_RULE.minimumBars)return 'HISTORIAL';
 if(card.seedDifference!=null&&Number.isFinite(card.seedDifference)&&card.seedDifference>=0&&card.seedDifference<=PRICE_STRUCTURE_RULE.seedTolerance&&(card.highs.length<2||card.lows.length<2))return 'FORMANDO';
 return 'SIN VERIFICAR';
}

export interface StructuralSessionReading {
 side:'CALL'|'PUT'|'MIXTA'|'PENDIENTE';availability:'current'|'reference'|'pending';
 closedAtMs:number|null;livePriceReady:boolean;subtitle:string;reason:string;
}

/** Reuse the existing verified 15m trend. Five-scale agreement remains context, not a new gate. */
export function structuralSessionReading(symbol:string,cards:readonly PriceStructureCard[],direction:Readonly<SessionDirectionEvidence>|undefined,now:number):StructuralSessionReading {
 const pending:StructuralSessionReading={side:'PENDIENTE',availability:'pending',closedAtMs:null,livePriceReady:false,subtitle:'15m · pendiente',reason:'Esperando un cierre técnico de 15m asociado a este activo. No confirma una entrada.'};
 const matching=cards.filter(c=>c.interval===15);
 if(!direction||direction.symbol!==symbol||matching.length!==1||!['current','reference'].includes(direction.availability)||!['CALL','PUT','MIXTA'].includes(direction.side)||!Number.isFinite(now)||!Number.isFinite(direction.closedAtMs)||direction.closedAtMs!<=0||direction.closedAtMs!>now||direction.closedAtMs!==matching[0].endMs)return pending;
 return {side:direction.side as 'CALL'|'PUT'|'MIXTA',availability:direction.availability as 'current'|'reference',closedAtMs:direction.closedAtMs,livePriceReady:direction.availability==='current'&&direction.livePriceReady,subtitle:direction.availability==='reference'?'15m · referencia':'15m · cierre verificado',reason:direction.reason};
}
