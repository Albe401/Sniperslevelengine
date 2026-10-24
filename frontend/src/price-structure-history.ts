import type {ChartHistory,ChartSession} from './candle-series';
export interface StructureDailyBar {date:string;open:number;high:number;low:number;close:number;volume:number}
export interface PriceStructureHistory {symbol:string;asOfMs:number;throughMs:number;sessions:ChartSession[];days:StructureDailyBar[];intraday:ChartHistory}
/** Replace overlapping sessions in full: a missing live minute remains missing, never patched. */
export function mergeStructureMinutes(older:ChartHistory,live:ChartHistory|undefined):ChartHistory {
 if(!live?.sessions.length)return older;
 const first=live.sessions[0].openMs;
 return {sessions:[...older.sessions.filter(s=>s.openMs<first),...live.sessions],minutes:[...older.minutes.filter(b=>b.timestampMs<first),...live.minutes]};
}
