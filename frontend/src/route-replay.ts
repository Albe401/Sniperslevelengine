import type {LiveState} from './live';
import {sessionCandles,type ChartHistory} from './candle-series';
import {readPriceNow,type PriceNow} from './price-now';
import {chart} from './chart';
import {escapeHtml as esc} from './model';

export interface ReplayPoint {at:number;reading:PriceNow;closed:number}
export interface RouteReplay {symbol:string;date:string;points:ReplayPoint[];gaps:number;ready:number;rejected:number;causal:boolean;scope:'price-only'}
/** Reconstructed close observation, not an original tick or historical options snapshot. */
export function historicalPriceState(history:ChartHistory,symbol:string,at:number):LiveState|undefined {
 const visible=sessionCandles(history,[],1,at).filter(b=>b.complete&&b.sessionDate===new Date(at-1).toLocaleDateString('en-CA',{timeZone:'America/New_York'}));
 const b=visible.at(-1);if(!b)return;
 return {symbol,publishedMs:at,stocks:{phase:'streaming'},options:{phase:'disconnected'},ticks:{[symbol]:{symbol,price:b.close,tradeMs:at}},metrics:{session:b.sessionDate,phase:'Sesión RTH',asofMs:at,vwap:b.sessionVwap},chartHistory:{sessions:history.sessions.filter(s=>s.openMs<at),minutes:history.minutes.filter(c=>c.timestampMs+60000<=at)},flow:{intraday_windows:[]}} as unknown as LiveState;
}
export function replayRoutePrice(history:ChartHistory,symbol:string,date:string,availableAt=Date.now()):RouteReplay {
 const bars=sessionCandles(history,[],1,availableAt).filter(b=>b.complete&&b.sessionDate===date);
 const out:RouteReplay={symbol,date,points:[],gaps:0,ready:0,rejected:0,causal:true,scope:'price-only'};
 for(let i=0;i<bars.length;i++){
  const bar=bars[i],at=bar.timestampMs+60000;
  if(i&&bar.timestampMs-bars[i-1].timestampMs!==60000)out.gaps++;
  const state=historicalPriceState(history,symbol,at);if(!state)continue;
  const reading=readPriceNow(state,symbol,true,at);
  // The complete history must agree with the truncated input at the same cutoff.
  const full={...state,chartHistory:history};
  if(JSON.stringify(readPriceNow(full,symbol,true,at))!==JSON.stringify(reading))out.causal=false;
  out.points.push({at,reading,closed:state.chartHistory!.minutes.length});
  if(reading.ready)out.ready++;else out.rejected++;
 }
 return out;
}
