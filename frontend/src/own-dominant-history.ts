import type {LiveState} from './live';
import {ownHeatDominants} from './own-heat-dominants';
import type {DominantMark} from './dominant-mark-painter';
export interface OwnGammaObservation {symbol:string;expiry:string;at:number;sourceMs:number;positive:number|null;negative:number|null;complete:boolean}
const formatter=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
const date=(t:number)=>formatter.format(t);
/** Recent display cache. Durable raw captures remain in the existing native snapshot archive. */
export class OwnDominantHistory {
 private entries=new Map<string,{last:string;points:OwnGammaObservation[]}>();
 observe(state:Pick<LiveState,'symbol'|'publishedMs'|'optionSnapshotStatus'>,receivedMs:number){
  const at=state.publishedMs;if(state.optionSnapshotStatus?.phase!=='ready')return;if(!/^[A-Z][A-Z0-9.]{0,19}$/.test(state.symbol)||!Number.isSafeInteger(at)||at<=0||!Number.isSafeInteger(receivedMs)||at>receivedMs)return;
  for(const h of state.optionSnapshotStatus?.horizons??[]){
   if(h.expirations.length!==1)continue;const expiry=h.expirations[0];if(!/^\d{4}-\d{2}-\d{2}$/.test(expiry))continue;
   const own=ownHeatDominants([h],expiry,[],at);if(!own)continue;
   const positive=own.positive?.strike??null,negative=own.negative?.strike??null,key=JSON.stringify([state.symbol,expiry,date(at)]),last=JSON.stringify([own.observedMs,positive,negative,own.complete]);
   const entry=this.entries.get(key)??{last:'',points:[]};if(entry.last===last)continue;
   const point:OwnGammaObservation={symbol:state.symbol,expiry,at,sourceMs:own.observedMs,positive,negative,complete:own.complete};
   const old=entry.points.find(p=>p.at===at);
   if(old){if(JSON.stringify(old)!==JSON.stringify(point)){old.positive=null;old.negative=null;old.complete=false;}}
   else entry.points.push(point);
   entry.last=last;entry.points.sort((a,b)=>a.at-b.at);if(entry.points.length>5000)entry.points.splice(0,entry.points.length-5000);this.entries.set(key,entry);
   if(this.entries.size>64)this.entries.delete(this.entries.keys().next().value!);
  }
 }
 all(symbol:string,asof:number):OwnGammaObservation[]{
  if(!Number.isSafeInteger(asof)||asof<=0)return [];
  const day=date(asof);return [...this.entries.values()].flatMap(e=>e.points).filter(p=>p.symbol===symbol&&p.at<=asof&&date(p.at)===day).map(p=>({...p})).sort((a,b)=>a.at-b.at);
 }
}
export const ownDominantHistory=new OwnDominantHistory();
export function collectOwnGammaMarks(state:LiveState){ownDominantHistory.observe(state,Date.now());}
/** Availability clock is the first received capture, never backdated to a source timestamp. */
export function ownDominantMarks(points:OwnGammaObservation[],symbol:string,expiry:string,asof:number,bars:number[],barMs:number):DominantMark[]{
 if(!Number.isSafeInteger(asof)||asof<=0||!Number.isSafeInteger(barMs)||barMs<=0||points.length>20000)return [];
 const day=date(asof),starts=[...new Set(bars.filter(t=>Number.isSafeInteger(t)&&t>=0))].sort((a,b)=>a-b),marks=new Map<string,DominantMark>();
 for(const p of points){if(p.symbol!==symbol||p.expiry!==expiry||!Number.isSafeInteger(p.at)||p.at<=0||p.at>asof||!Number.isSafeInteger(p.sourceMs)||p.sourceMs<=0||p.sourceMs>p.at||p.at-p.sourceMs>300000||date(p.at)!==day)continue;
  let lo=0,hi=starts.length;while(lo<hi){const mid=(lo+hi)>>>1;if(starts[mid]<=p.at)lo=mid+1;else hi=mid;}const bar=starts[lo-1];if(lo===0||p.at>=bar+barMs)continue;
  for(const [side,price]of [['positive',p.positive],['negative',p.negative]]as const){if(typeof price!=='number'||!Number.isFinite(price)||price<=0)continue;
   const key=JSON.stringify([bar,side,price,!p.complete]),previous=marks.get(key);if(!previous||p.at>previous.at)marks.set(key,{side,price,bar,at:p.at,fraction:(p.at-bar)/barMs,provisional:!p.complete});
  }
 }
 return [...marks.values()].sort((a,b)=>a.bar-b.bar||a.at-b.at||a.side.localeCompare(b.side));
}
