import {invoke,isTauri} from './snapshot-observation';
import type {ChartHistory} from './candle-series';
import type {AuctionReading,AuctionBin} from './auction-map-panel';
export interface AuctionReference {symbol:string;date:string;close:number;reading:AuctionReading}
const dateET=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
/** Historical-only OHLCV approximation. Never certified tick volume or a live route. */
export function historicalAuction(symbol:string,history:ChartHistory|undefined,now:number):AuctionReference|undefined {
 if(!history||!Number.isFinite(now))return;
 const session=history.sessions.filter(s=>Number.isFinite(s.openMs)&&Number.isFinite(s.closeMs)&&s.closeMs<=now&&s.closeMs>s.openMs&&s.closeMs-s.openMs<=390*60000&&dateET.format(s.openMs)===s.date).sort((a,b)=>b.closeMs-a.closeMs)[0];
 if(!session||now-session.closeMs>14*86400000)return;
 const bars=history.minutes.filter(b=>b.timestampMs>=session.openMs&&b.timestampMs<session.closeMs).sort((a,b)=>a.timestampMs-b.timestampMs);
 // Need the actual final regular-session minute to label its price as the close.
 if(!bars.length||bars.at(-1)!.timestampMs+60000!==session.closeMs)return;
 // A final close alone cannot certify the volume profile of a whole session.
 if(bars.length!==(session.closeMs-session.openMs)/60000||bars.some((b,i)=>b.timestampMs!==session.openMs+i*60000))return;
 if(bars.some((b,i)=>![b.timestampMs,b.open,b.high,b.low,b.close,b.volume].every(Number.isFinite)||b.low<=0||b.volume<0||b.low>b.open||b.low>b.close||b.high<b.open||b.high<b.close||b.low>b.high||b.timestampMs%60000!==0||(i>0&&b.timestampMs===bars[i-1].timestampMs)))return;
 const lo=Math.min(...bars.map(b=>b.low)),hi=Math.max(...bars.map(b=>b.high));
 const size=Math.max(.01,Math.ceil((hi-lo)/120/.01)*.01);
 const first=Math.floor(lo/size+1e-9),last=Math.floor(hi/size+1e-9);
 const rows=Array.from({length:last-first+1},()=>({volume:0,blocks:new Set<number>()}));
 for(const b of bars){const a=Math.floor(b.low/size+1e-9),z=Math.floor(b.high/size+1e-9);for(let k=a;k<=z;k++){const row=rows[k-first];row.volume+=b.volume/(z-a+1);row.blocks.add(Math.floor((b.timestampMs-session.openMs)/1800000));}}
 const total=rows.reduce((n,r)=>n+r.volume,0);if(!(total>0&&Number.isFinite(total)))return;
 const poc=rows.reduce((best,r,i)=>r.volume>rows[best].volume?i:best,0),time=Math.max(...rows.map(r=>r.blocks.size));
 let left=poc,right=poc,volume=rows[poc].volume;
 while(volume<.7*total&&(left>0||right+1<rows.length)){if(left>0&&(right+1===rows.length||rows[left-1].volume>=rows[right+1].volume)){volume+=rows[--left].volume;}else volume+=rows[++right].volume;}
 const bins:AuctionBin[]=rows.map((r,i)=>{const v=r.volume/rows[poc].volume,t=r.blocks.size/time;return {low:(first+i)*size,high:(first+i+1)*size,volume:r.volume,brackets:r.blocks.size,volumeRatio:v,timeRatio:t,kind:v>=.5&&t>=.5&&r.blocks.size>=2?'acceptance':r.volume===0?'untraded':v<=.2&&t<=.5?'low-participation':'transition'};});
 return {symbol,date:session.date,close:bars.at(-1)!.close,reading:{version:'auction-map-bars-reference-v1',symbol,asOfMs:session.closeMs,provisional:true,acceptedTrades:0,totalVolume:total,pocLow:bins[poc].low,pocHigh:bins[poc].high,val:bins[left].low,vah:bins[right].high,bins,reaction:'pending',routes:[],calibrated:false}};
}
const cache=new Map<string,ChartHistory>(),pending=new Set<string>(),retry=new Map<string,number>(),checked=new Map<string,string>();
/** Can load while disconnected; exclusively uses the existing native history endpoint. */
export function auctionHistoryReference(symbol:string,history:ChartHistory|undefined,now:number):AuctionReference|undefined {
 const cached=cache.get(symbol),direct=historicalAuction(symbol,history,now),saved=historicalAuction(symbol,cached,now);
 if(isTauri()&&!pending.has(symbol)&&checked.get(symbol)!==dateET.format(now)&&now>=(retry.get(symbol)??0)){
  pending.add(symbol);
  void invoke<ChartHistory>('get_extended_chart_history',{symbol}).then(data=>{
   const reference=historicalAuction(symbol,data,Date.now());if(!reference)throw Error('No complete historical close');
   cache.set(symbol,data);checked.set(symbol,dateET.format(Date.now()));
   document.dispatchEvent(new CustomEvent('snipers:auction-history'));
  }).catch(()=>retry.set(symbol,Date.now()+60000)).finally(()=>pending.delete(symbol));
 }
 return direct&&(!saved||direct.reading.asOfMs>=saved.reading.asOfMs)?direct:saved;
}
