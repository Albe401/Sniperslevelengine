import type {LiveState} from './live';
import type {Candle} from './model';
import {sessionCandles,type SessionCandle,type ChartHistory} from './candle-series';
/** Canonical provider candles; current gamma is never presented as past gamma. */
export function gammaChartData(s:LiveState|undefined,symbol:string,active:boolean,interval:number,now=Date.now(),history:ChartHistory|undefined=s?.chartHistory) {
 if(![1,2,5,15,60,240,1440].includes(interval))throw new Error('Temporalidad no admitida');
 const source=active&&s?.symbol===symbol?sessionCandles(history,Object.values(s.bars??{}),1,now):[];
 const session=source.at(-1)?.sessionDate;
 const day=new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
 const historical=session!=null&&session!==day;
 const selected=source;
 const definitions=new Map(history?.sessions.map(x=>[x.date,x])??[]);
 const key=(b:SessionCandle)=>{if(interval===1440)return Date.parse(b.sessionDate+'T12:00:00Z');const anchor=definitions.get(b.sessionDate)?.openMs??b.timestampMs;return anchor+Math.floor((b.timestampMs-anchor)/(interval*60000))*interval*60000;};
 const groups=new Map<number,Candle>();
 for(const b of selected){const t=key(b),old=groups.get(t);if(old){old.high=Math.max(old.high,b.high);old.low=Math.min(old.low,b.low);old.close=b.close;old.volume+=b.volume;}else groups.set(t,{...b,timestampMs:t});}
 const bars:Candle[]=interval<=15&&active&&s?.symbol===symbol?sessionCandles(history,Object.values(s.bars??{}),interval,now):[...groups.values()];
 const bucket=new Map<number,SessionCandle>(),membersByBucket=new Map<number,SessionCandle[]>();for(const b of selected){const t=key(b);bucket.set(t,b);const members=membersByBucket.get(t)??[];members.push(b);membersByBucket.set(t,members);}
 const vwap=bars.map(b=>{const last=interval<=15?b as SessionCandle:bucket.get(b.timestampMs);return {timestampMs:b.timestampMs,value:last?.sessionVwap??null};});
 const closedBars=bars.filter(b=>{
  if(interval<=15)return (b as SessionCandle).complete;
  const members=membersByBucket.get(b.timestampMs)??[],definition=definitions.get(members[0]?.sessionDate);
  const bucketEnd=interval===240&&definition?Math.min(b.timestampMs+interval*60000,definition.closeMs):b.timestampMs+interval*60000;
  const expected=interval===240?(bucketEnd-b.timestampMs)/60000:interval===1440&&definition?(definition.closeMs-definition.openMs)/60000:interval;
  const end=interval===1440?definition?.closeMs:bucketEnd;
  return expected>0&&members.length===expected&&members.every((m,i)=>m.complete&&(i===0||m.timestampMs-members[i-1].timestampMs===60000))&&end!=null&&end<=now;
 }).map(b=>interval===240?{...b,closedAtMs:Math.min(b.timestampMs+interval*60000,definitions.get((b as SessionCandle).sessionDate)?.closeMs??b.timestampMs+interval*60000)}:b);
 return {closedBars,bars,vwap,session,historical,currentOverlay:source.length>0&&!historical,lastBarMs:source.at(-1)?.timestampMs??null};
}
