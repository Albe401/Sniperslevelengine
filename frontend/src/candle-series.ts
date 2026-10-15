import {reconcileMinutes,type Candle} from './model';
export interface RawMinute extends Candle {vwap?:number|null}
export interface ChartSession {date:string;openMs:number;closeMs:number}
export interface ChartHistory {minutes:RawMinute[];sessions:ChartSession[]}
export interface SessionCandle extends Candle {sessionDate:string;sessionVwap:number|null;minutesReceived:number;complete:boolean;provisional:boolean}
const sessionDate=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
/** Intraday engines need today's calendar only; historical chart data is unchanged. */
export function currentSessionCandles(history:ChartHistory|undefined,period:number,now=Date.now()):SessionCandle[]{
 if(!history||!Number.isFinite(now))return [];
 const sessions=history.sessions.filter(s=>s.date===sessionDate.format(now));
 if(sessions.length!==1)return [];
 const session=sessions[0];
 if(![session.openMs,session.closeMs].every(Number.isFinite)||session.closeMs<=session.openMs)return [];
 return sessionCandles({sessions,minutes:history.minutes.filter(b=>b.timestampMs>=session.openMs&&b.timestampMs<session.closeMs)},[],period,now);
}
export function sessionCandles(history:ChartHistory|undefined,seconds:Candle[],period:number,now=Date.now()):SessionCandle[]{
 if(!history||![1,2,5,15].includes(period))return [];
 const valid=(c:Candle)=>[c.timestampMs,c.open,c.high,c.low,c.close,c.volume].every(Number.isFinite)&&c.timestampMs<=now&&c.volume>=0&&c.low>0&&c.high>=Math.max(c.open,c.close)&&c.low<=Math.min(c.open,c.close);
 const canonical=new Map(history.minutes.filter(valid).map(c=>[c.timestampMs,c]));
 const merged=reconcileMinutes([...canonical.values()],seconds.filter(valid));
 const out:SessionCandle[]=[];
 for(const session of history.sessions){
  let expected=session.openMs,numerator=0,volume=0,known=true;
  const groups=new Map<number,SessionCandle>();
  for(const row of merged.filter(c=>c.timestampMs>=session.openMs&&c.timestampMs<session.closeMs)){
   const raw=canonical.get(row.timestampMs),closed=row.timestampMs+60000<=now;
   if(row.timestampMs!==expected||!raw||!closed)known=false;
   expected=row.timestampMs+60000;
   if(row.volume>0){if(raw?.vwap!=null&&Number.isFinite(raw.vwap)&&raw.vwap>0){numerator+=raw.vwap*row.volume;volume+=row.volume;}else known=false;}
   const vwap=known&&volume>0?numerator/volume:null;
   const t=session.openMs+Math.floor((row.timestampMs-session.openMs)/(period*60000))*period*60000;
   const old=groups.get(t);
   if(old){old.high=Math.max(old.high,row.high);old.low=Math.min(old.low,row.low);old.close=row.close;old.volume+=row.volume;old.minutesReceived++;old.sessionVwap=vwap;old.provisional ||= !raw||!closed;}
   else groups.set(t,{...row,timestampMs:t,sessionDate:session.date,sessionVwap:vwap,minutesReceived:1,complete:false,provisional:!raw||!closed});
  }
  for(const row of groups.values()){row.complete=!row.provisional&&row.minutesReceived===period&&row.timestampMs+period*60000<=Math.min(now,session.closeMs);out.push(row);}
 }
 return out.sort((a,b)=>a.timestampMs-b.timestampMs);
}
