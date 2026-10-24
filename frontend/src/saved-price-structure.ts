import type {LiveState} from './live';
import type {PriceStructureHistory} from './price-structure-history';
import {PRICE_INTERVALS,PRICE_STRUCTURE_RULE,priceStructureCards,type PriceStructureCard} from './price-structure';

/** Frozen output of the same native kernel. No browser market calls or majority signals. */
export const STRUCTURE_VERSION='price-structure-five-wilder14-pivots2-v1';
export interface SavedPriceStructure {
 version:typeof STRUCTURE_VERSION;symbol:string;capturedMs:number;publishedMs:number;
 historyAsOfMs:number|null;historySha256:string|null;cards:PriceStructureCard[];
}
const finite=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n);
const date=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
export function capturePriceStructure(frame:LiveState,symbol:string,capturedMs:number,history?:PriceStructureHistory,sha:string|null=null):SavedPriceStructure {
 const valid=history?.symbol===symbol&&history.asOfMs<=capturedMs&&date.format(history.asOfMs)===date.format(capturedMs)?history:undefined;
 return {version:STRUCTURE_VERSION,symbol,capturedMs,publishedMs:frame.publishedMs,
  historyAsOfMs:valid?.asOfMs??null,historySha256:valid?sha:null,
  cards:priceStructureCards(frame,symbol,true,capturedMs,frame.chartHistory,valid)};
}
/** Validate stored identity, causal clock, numerical evidence and classification before rendering. */
export function savedPriceStructure(frame:LiveState|undefined,symbol:string,capturedMs:number):PriceStructureCard[]|undefined {
 const raw=(frame as (LiveState&{priceStructureReading?:SavedPriceStructure})|undefined)?.priceStructureReading;
 if(!frame||!raw||raw.version!==STRUCTURE_VERSION||raw.symbol!==symbol||frame.symbol!==symbol
  ||raw.capturedMs!==capturedMs||raw.publishedMs!==frame.publishedMs||!finite(raw.publishedMs)
  ||raw.publishedMs>capturedMs||capturedMs-raw.publishedMs>15000
  ||raw.historyAsOfMs!==null&&(!finite(raw.historyAsOfMs)||raw.historyAsOfMs>capturedMs||date.format(raw.historyAsOfMs)!==date.format(capturedMs))
  ||raw.historySha256!==null&&!/^[a-f0-9]{64}$/.test(raw.historySha256)
  ||!Array.isArray(raw.cards)||raw.cards.length!==5)return;
 for(let i=0;i<5;i++){
  const c=raw.cards[i];
  if(!c||c.interval!==PRICE_INTERVALS[i]||!['CALL','PUT','MIXTA','SIN DATOS'].includes(c.side)
   ||!Number.isSafeInteger(c.bars)||c.bars<0||!Number.isSafeInteger(c.gaps)||c.gaps<0
   ||typeof c.invalidated!=='boolean'||typeof c.reference!=='boolean'
   ||['name','role','reason'].some(k=>typeof c[k as 'name']!=='string'||c[k as 'name'].length>400)
   ||[c.atr,c.highChange,c.lowChange,c.lastClose,c.endMs,c.lastDuration,c.seedDifference].some(v=>v!==null&&!finite(v))
   ||c.endMs!==null&&(c.endMs>capturedMs||c.endMs<=0)
   ||!Array.isArray(c.highs)||!Array.isArray(c.lows)||c.highs.length>2||c.lows.length>2)return;
  for(const [kind,pivots] of [['high',c.highs],['low',c.lows]] as const){
   if(pivots.some((p,j)=>!p||p.kind!==kind||!finite(p.price)||p.price<=0||!Number.isSafeInteger(p.index)||p.index<2||p.index>=c.bars-2
    ||!finite(p.at)||!finite(p.confirmedMs)||p.at>=p.confirmedMs||c.endMs===null||p.confirmedMs>c.endMs
    ||j>0&&(p.at<=pivots[j-1].at||p.index<=pivots[j-1].index)))return;
  }
  if(c.side==='SIN DATOS')continue;
  if(c.bars<PRICE_STRUCTURE_RULE.minimumBars||c.atr===null||c.atr<=0||c.seedDifference===null||c.seedDifference<0||c.seedDifference>PRICE_STRUCTURE_RULE.seedTolerance
   ||c.highs.length!==2||c.lows.length!==2||c.highChange===null||c.lowChange===null||c.endMs===null||c.lastClose===null||c.lastClose<=0
   ||c.interval>=1440&&(raw.historyAsOfMs===null||raw.historySha256===null))return;
  const high=(c.highs[1].price-c.highs[0].price)/c.atr,low=(c.lows[1].price-c.lows[0].price)/c.atr;
  if(Math.abs(high-c.highChange)>1e-10||Math.abs(low-c.lowChange)>1e-10)return;
  const expected=c.invalidated?'MIXTA':high>0&&low>0?'CALL':high<0&&low<0?'PUT':'MIXTA';
  if(c.side!==expected)return;
 }
 return raw.cards;
}
