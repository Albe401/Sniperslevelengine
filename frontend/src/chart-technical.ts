import type {Candle} from './model';
export interface ChartPoint {timestampMs:number;value:number}
export interface StructureMark {timestampMs:number;confirmedMs:number;price:number;text:string;up:boolean;above:boolean}
/** Plot on the confirming bar, never where a pivot was unknowable in real time. */
export function technicalChartMarks(marks:StructureMark[]){return marks.slice(-12).map(p=>({timestampMs:p.confirmedMs,above:p.above,color:'#9bacc5',shape:'circle' as const,text:p.text}));}
/** OHLCV indicators only; no inferred option purchases or probability output. */
export function chartTechnical(input:(Candle&{closedAtMs?:number})[],interval:number,now:number) {
 let bars:Candle[]=[];
 for(const b of input.slice(-600)){
  if(Number.isFinite(b.timestampMs)&&(b.closedAtMs??(interval<1440?b.timestampMs+interval*60000:b.timestampMs))>now)break;
  if(![b.timestampMs,b.open,b.high,b.low,b.close,b.volume].every(Number.isFinite)||b.volume<0||b.low<=0||b.low>Math.min(b.open,b.close)||b.high<Math.max(b.open,b.close)||(b.closedAtMs??(interval<1440?b.timestampMs+interval*60000:b.timestampMs))>now){bars=[];continue;}
  const prev=bars.at(-1);const otherSession=prev&&interval===240&&new Date(prev.timestampMs).toLocaleDateString('en-CA',{timeZone:'America/New_York'})!==new Date(b.timestampMs).toLocaleDateString('en-CA',{timeZone:'America/New_York'});if(prev&&(b.timestampMs<=prev.timestampMs||(interval<1440&&!otherSession&&b.timestampMs-prev.timestampMs!==interval*60000)))bars=[];
  bars.push(b);
 }
 const closes=bars.map(b=>b.close),mean=(v:number[])=>v.reduce((a,b)=>a+b,0)/v.length;
 const upper:ChartPoint[]=[],middle:ChartPoint[]=[],lower:ChartPoint[]=[];
 for(let i=19;i<bars.length;i++){const w=closes.slice(i-19,i+1),m=mean(w),sd=Math.sqrt(mean(w.map(v=>(v-m)**2)));const t=bars[i].timestampMs;upper.push({timestampMs:t,value:m+2*sd});middle.push({timestampMs:t,value:m});lower.push({timestampMs:t,value:m-2*sd});}
 const ema=(v:number[],n:number)=>{const out:(number|null)[]=v.map(()=>null);if(v.length<n)return out;let x=mean(v.slice(0,n));out[n-1]=x;for(let i=n;i<v.length;i++){x+=(v[i]-x)*2/(n+1);out[i]=x;}return out;};
 let rsi:number|null=null,atr:number|null=null;
 if(bars.length>=15){const changes=closes.slice(1).map((c,i)=>c-closes[i]),trs=bars.slice(1).map((c,i)=>Math.max(c.high-c.low,Math.abs(c.high-bars[i].close),Math.abs(c.low-bars[i].close)));let gain=mean(changes.slice(0,14).map(v=>Math.max(0,v))),loss=mean(changes.slice(0,14).map(v=>Math.max(0,-v)));atr=mean(trs.slice(0,14));for(let i=14;i<changes.length;i++){gain=(gain*13+Math.max(0,changes[i]))/14;loss=(loss*13+Math.max(0,-changes[i]))/14;atr=(atr*13+trs[i])/14;}rsi=loss===0?(gain===0?50:100):100-100/(1+gain/loss);}
 const fast=ema(closes,12),slow=ema(closes,26),macds=closes.map((_,i)=>fast[i]!=null&&slow[i]!=null?fast[i]!-slow[i]!:null).filter((v):v is number=>v!=null),signals=ema(macds,9),macd=macds.at(-1)??null,signal=signals.at(-1)??null;
 const typical=bars.map(b=>(b.high+b.low+b.close)/3);let mfi:number|null=null,cmf:number|null=null,obv=0;
 if(bars.length>=15){let buy=0,sell=0;for(let i=bars.length-14;i<bars.length;i++){const amount=typical[i]*bars[i].volume;if(typical[i]>typical[i-1])buy+=amount;else if(typical[i]<typical[i-1])sell+=amount;}mfi=buy+sell===0?null:sell===0?100:100-100/(1+buy/sell);}
 if(bars.length>=20){const tail=bars.slice(-20),v=tail.reduce((a,c)=>a+c.volume,0);cmf=v>0?tail.reduce((a,c)=>a+(c.high===c.low?0:(2*c.close-c.high-c.low)/(c.high-c.low))*c.volume,0)/v:null;}
 for(let i=1;i<bars.length;i++)obv+=Math.sign(bars[i].close-bars[i-1].close)*bars[i].volume;
 const base=bars.length>=21?mean(bars.slice(-21,-1).map(c=>c.volume)):0,relativeVolume=base>0?bars.at(-1)!.volume/base:null;
 const marks:StructureMark[]=[];let high:StructureMark|undefined,low:StructureMark|undefined,priorHigh:number|undefined,priorLow:number|undefined,brokenHigh=false,brokenLow=false,lastBreak=0;
 for(let j=4;j<bars.length;j++){const i=j-2,p=bars[i],neighbors=bars.slice(i-2,i+3).filter((_,n)=>n!==2);
  if(neighbors.every(c=>p.high>c.high)){const text=priorHigh==null?'H':p.high>priorHigh?'HH':'LH';high={timestampMs:p.timestampMs,confirmedMs:bars[j].timestampMs,price:p.high,text,up:text==='HH',above:true};marks.push(high);priorHigh=p.high;brokenHigh=false;}
  if(neighbors.every(c=>p.low<c.low)){const text=priorLow==null?'L':p.low>priorLow?'HL':'LL';low={timestampMs:p.timestampMs,confirmedMs:bars[j].timestampMs,price:p.low,text,up:text==='HL',above:false};marks.push(low);priorLow=p.low;brokenLow=false;}
  for(const [pivot,up,broken] of [[high,true,brokenHigh],[low,false,brokenLow]] as const){if(!pivot||broken||bars[j-1].timestampMs<pivot.confirmedMs)continue;const tolerance=pivot.price*.0003,sign=up?1:-1;if((bars[j-1].close-pivot.price)*sign>tolerance&&(bars[j].close-pivot.price)*sign>tolerance){marks.push({timestampMs:bars[j].timestampMs,confirmedMs:bars[j].timestampMs,price:bars[j].close,text:lastBreak&&lastBreak!==sign?'ChoCH?':'BOS',up,above:!up});lastBreak=sign;if(up)brokenHigh=true;else brokenLow=true;}}
 }
 return {bars,upper,middle,lower,marks:marks.slice(-60),rsi,atr,macd,signal,histogram:macd!=null&&signal!=null?macd-signal:null,mfi,cmf,obv:bars.length>=2?obv:null,relativeVolume};
}
