import type {PriceBar} from './price-structure';

/** Wilder DM: equal expansions contribute to neither side. Bound only binary price
 * representation/subtraction error; this is not a market-direction threshold.
 * Four ulps at the input price scale conservatively cover both input differences.
 */
export function directionalMovement(b:Pick<PriceBar,'high'|'low'>,p:Pick<PriceBar,'high'|'low'>):{up:number;down:number} {
 const u=b.high-p.high,d=p.low-b.low;
 const resolution=4*Number.EPSILON*Math.max(1,Math.abs(b.high),Math.abs(p.high),Math.abs(b.low),Math.abs(p.low));
 return {up:u>0&&u-d>resolution?u:0,down:d>0&&d-u>resolution?d:0};
}

export interface TechnicalRead {ema9:number|null;ema20:number|null;ema21:number|null;ema21ThreeAgo:number|null;plusDi:number|null;minusDi:number|null;adx:number|null}
interface State extends TechnicalRead {count:number;sum9:number;sum20:number;sum21:number;tr:number;plus:number;minus:number;dxSum:number;dxCount:number;tail:number[];previous:PriceBar|null}
const cache=new Map<string,{bars:PriceBar[];state:State}>();
let hits=0,appends=0,resets=0,processedBars=0;
const initial=():State=>({count:0,sum9:0,sum20:0,sum21:0,tr:0,plus:0,minus:0,dxSum:0,dxCount:0,tail:[],previous:null,ema9:null,ema20:null,ema21:null,ema21ThreeAgo:null,plusDi:null,minusDi:null,adx:null});
const same=(a:PriceBar,b:PriceBar)=>a.timestampMs===b.timestampMs&&a.endMs===b.endMs&&a.open===b.open&&a.high===b.high&&a.low===b.low&&a.close===b.close;
function advance(s:State,b:PriceBar):void {
 const index=s.count;
 for(const period of [9,20,21] as const){const sum=('sum'+period) as 'sum9'|'sum20'|'sum21',ema=('ema'+period)as 'ema9'|'ema20'|'ema21';
  if(index<period){s[sum]+=b.close;if(index===period-1)s[ema]=s[sum]/period;}else{s[ema]!+=2/(period+1)*(b.close-s[ema]!);}
 }
 if(s.ema21!==null){s.tail.push(s.ema21);if(s.tail.length>4)s.tail.shift();}s.ema21ThreeAgo=s.tail.length===4?s.tail[0]:null;
 if(s.previous){const p=s.previous,t=Math.max(b.high-b.low,Math.abs(b.high-p.close),Math.abs(b.low-p.close)),{up,down}=directionalMovement(b,p);
  if(index<=14){s.tr+=t;s.plus+=up;s.minus+=down;}else{s.tr=s.tr-s.tr/14+t;s.plus=s.plus-s.plus/14+up;s.minus=s.minus-s.minus/14+down;}
  if(index>=14){s.plusDi=s.tr>0?100*s.plus/s.tr:0;s.minusDi=s.tr>0?100*s.minus/s.tr:0;const dx=s.plusDi+s.minusDi>0?100*Math.abs(s.plusDi-s.minusDi)/(s.plusDi+s.minusDi):0;
   if(s.dxCount<14){s.dxSum+=dx;s.dxCount++;if(s.dxCount===14)s.adx=s.dxSum/14;}else{s.adx=(s.adx!*13+dx)/14;}
  }
 }
 s.previous={...b};s.count++;processedBars++;
}
/** Bounded exact-prefix cache: no hashes, assumed immutability or final-price-only shortcuts.
 * A correction, truncation, shifted origin or clock change rebuilds the affected scale.
 */
export function cachedTechnical(bars:readonly PriceBar[],key:string):TechnicalRead {
 let record=cache.get(key),prefix=!!record&&bars.length>=record.bars.length&&record.bars.every((b,i)=>same(b,bars[i]));
 if(!prefix){record={bars:[],state:initial()};resets++;}else if(record!.bars.length===bars.length){hits++;}else{appends++;}
 for(let i=record!.bars.length;i<bars.length;i++){advance(record!.state,bars[i]);record!.bars.push({...bars[i]});}
 cache.delete(key);cache.set(key,record!);while(cache.size>12)cache.delete(cache.keys().next().value!);
 const s=record!.state;return {ema9:s.ema9,ema20:s.ema20,ema21:s.ema21,ema21ThreeAgo:s.ema21ThreeAgo,plusDi:s.plusDi,minusDi:s.minusDi,adx:s.adx};
}
export function structuralIndicatorCacheStats(){return {hits,appends,resets,processedBars,entries:cache.size,retainedBars:[...cache.values()].reduce((n,c)=>n+c.bars.length,0)};}
export function resetStructuralIndicatorCache():void{cache.clear();hits=0;appends=0;resets=0;processedBars=0;}
