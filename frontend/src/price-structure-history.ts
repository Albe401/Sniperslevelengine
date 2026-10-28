import type {ChartHistory,ChartSession} from './candle-series';
export interface StructureDailyBar {date:string;open:number;high:number;low:number;close:number;volume:number}
export interface PriceStructureHistory {symbol:string;asOfMs:number;throughMs:number;sessions:ChartSession[];days:StructureDailyBar[];intraday:ChartHistory}
/** Replace overlapping sessions in full: a missing live minute remains missing, never patched. */
export function mergeStructureMinutes(older:ChartHistory,live:ChartHistory|undefined):ChartHistory {
 if(!live?.sessions.length)return older;
 const first=live.sessions[0].openMs;
 return {sessions:[...older.sessions.filter(s=>s.openMs<first),...live.sessions],minutes:[...older.minutes.filter(b=>b.timestampMs<first),...live.minutes]};
}

const adjustedCloseIndex=new WeakMap<ChartHistory,Map<number,number>>();

/** Fail closed when overlapping REST split-adjusted minutes and live minutes have
 * a persistent price-scale conflict. This is a source-integrity guard, not a
 * split detector: late corrections can also cause disagreement. Missing live
 * minutes still remain missing in mergeStructureMinutes.
 */
export function structurePriceBasisConflict(older:ChartHistory,live:ChartHistory|undefined,availableThroughMs=Number.POSITIVE_INFINITY):boolean {
 if(!live?.sessions.length)return false;
 let reference=adjustedCloseIndex.get(older);
 if(!reference){reference=new Map(older.minutes.map(b=>[b.timestampMs,b.close]));adjustedCloseIndex.set(older,reference);}
 let run=0,lastRatio=0;
 for(const bar of live.minutes){
  // This integrity decision must use the same closed-bar availability as the
  // structural calculation; a future/forming minute cannot withdraw a past read.
  if(bar.timestampMs+60000>availableThroughMs)continue;
  const prior=reference.get(bar.timestampMs);
  if(prior==null)continue;
  if(!Number.isFinite(prior)||!Number.isFinite(bar.close)||prior<=0||bar.close<=0)return true;
  const ratio=bar.close/prior;
  if(Math.abs(ratio-1)>=.2)return true;
  if(Math.abs(ratio-1)>=.01&&Math.abs(ratio-lastRatio)<=.003)run++;
  else run=Math.abs(ratio-1)>=.01?1:0;
  lastRatio=ratio;
  if(run>=3)return true;
 }
 return false;
}
