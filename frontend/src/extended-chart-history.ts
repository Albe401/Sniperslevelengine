import {invoke} from './snapshot-observation';
import type {ChartHistory} from './candle-series';
import type {LiveState} from './live';

let cache: {symbol:string;date:string;history:ChartHistory}|undefined;
let pending = '';
let retryAfter = 0;
const etDate = new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});

/** Older bars are fetched once per asset/day, outside the live market update channel. */
export function extendedChartHistory(s:LiveState|undefined,symbol:string,active:boolean,refresh:()=>void,now=Date.now()):ChartHistory|undefined {
 if(!active||s?.symbol!==symbol)return s?.chartHistory;
 const date=etDate.format(now),key=`${symbol}:${date}`;
 if(cache?.symbol!==symbol||cache.date!==date){
  if(pending!==key&&now>=retryAfter){
   pending=key;
   void invoke<ChartHistory>('get_extended_chart_history',{symbol}).then(history=>{
    if(pending!==key)return;
    if(history.sessions.length&&history.minutes.length)cache={symbol,date,history};
    pending='';refresh();
   }).catch(()=>{if(pending===key)pending='';retryAfter=Date.now()+60_000;});
  }
  return s.chartHistory;
 }
 const older=cache.history,live=s.chartHistory;
 if(!live)return older;
 const lastLive=live.sessions[0]?.date;
 const sessions=[...older.sessions.filter(x=>!lastLive||x.date<lastLive),...live.sessions];
 const dates=new Set(sessions.map(x=>x.date));
 const replacementStart=live.minutes[0]?.timestampMs??Infinity;
 const minutes=[...older.minutes.filter(x=>x.timestampMs<replacementStart),...live.minutes];
 return {sessions:sessions.filter(x=>dates.delete(x.date)),minutes};
}
