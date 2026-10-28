import {validStructuralIndicators} from './structural-indicators';
import {PRICE_STRUCTURE_RULE,type PriceStructureCard,type FiveMinuteStructureCard} from './price-structure';
import type {StructuralSessionReading} from './structural-session-reading';

export interface SessionStructurePair {
 version:'session-structure-pair-v1';symbol:string;availability:'current'|'reference'|'pending';
 side:'CALL'|'PUT'|'MIXTA'|'PARCIAL';label:string;five:FiveMinuteStructureCard;fifteenCloseMs:number|null;
 trendFive:string;trendFifteen:string;technicalAgreement:boolean;reason:string;
 probability:null;entryAuthorized:false;
}
/** Descriptive comparison of overlapping closed-price structures, never independent votes or an entry gate. */
export function sessionStructurePair(symbol:string,five:FiveMinuteStructureCard,fifteen:PriceStructureCard|undefined,reading:Readonly<StructuralSessionReading>,now:number):SessionStructurePair {
 const out:SessionStructurePair={version:'session-structure-pair-v1',symbol,availability:'pending',side:'PARCIAL',label:'5m/15m · parcial',five,fifteenCloseMs:fifteen?.endMs??null,trendFive:'PENDIENTE',trendFifteen:reading.side,technicalAgreement:false,reason:'Esperando estructuras de 5m y 15m verificables del mismo corte.',probability:null,entryAuthorized:false};
 const valid=(c:PriceStructureCard<5>|PriceStructureCard|undefined)=>!!c&&['CALL','PUT','MIXTA'].includes(c.side)&&Number.isFinite(c.endMs)&&c.endMs!>0&&c.endMs!<=now&&c.bars>=PRICE_STRUCTURE_RULE.minimumBars&&c.atr!=null&&Number.isFinite(c.atr)&&c.atr>0&&c.seedDifference!=null&&c.seedDifference>=0&&c.seedDifference<=PRICE_STRUCTURE_RULE.seedTolerance&&c.highs.length===2&&c.lows.length===2&&[...c.highs,...c.lows].every(p=>p.confirmedMs<=c.endMs!&&p.at<p.confirmedMs);
 if(five.sourceSymbol!==symbol||!Number.isFinite(five.sourcePublishedMs)||five.sourcePublishedMs!>now||now-five.sourcePublishedMs!>15000||five.endMs!=null&&five.endMs>five.sourcePublishedMs!||five.interval!==5||fifteen?.interval!==15||!Number.isFinite(now)||reading.availability==='pending'||reading.closedAtMs!==fifteen.endMs||!valid(five)||!valid(fifteen)||five.endMs!<fifteen.endMs!||five.endMs!-fifteen.endMs!>=900000)return {...out,reason:five.side==='SIN DATOS'?five.reason:out.reason};
 out.availability=reading.availability;out.side=five.side===fifteen.side&&five.side!=='MIXTA'?five.side as 'CALL'|'PUT':'MIXTA';
 out.label=out.side==='MIXTA'?'5m/15m · transición':'5m/15m · coinciden '+out.side;
 out.trendFive=validStructuralIndicators(five.indicators,five.endMs,five.bars)&&five.indicators!.ready?five.indicators!.trend:'PENDIENTE';
 out.technicalAgreement=out.side!=='MIXTA'&&out.trendFive===out.side&&reading.side===out.side;
 out.reason='Comparación descriptiva de pivotes: 5m '+five.side+'; 15m '+fifteen.side+'. '+(out.technicalAgreement?'Las tendencias técnicas también coinciden.':'La tendencia técnica puede diferir de los pivotes.')+' Las escalas comparten precios; no son votos independientes ni confirman una entrada.';
 return out;
}
