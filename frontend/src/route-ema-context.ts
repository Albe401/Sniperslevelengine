import type {LiveState} from './live';
/** Same SMA seed and alpha=2/(period+1) as bot_ema.rs; observational, closed RTH minutes only. */
export function routeEmaContext(s:LiveState|undefined,now:number){
 const empty={available:false,ema9:null as number|null,ema20:null as number|null,ema50:null as number|null,ema200:null as number|null,cross:0,rebound9:0,rebound20:0,label:'EMA 9/20/50/200 · esperando 200 cierres RTH continuos'};
 if(!Number.isFinite(now)||!s||s.stocks?.phase!=='streaming'||!Number.isFinite(s.publishedMs)||s.publishedMs>now||now-s.publishedMs>3000)return empty;
 const sessions=s.chartHistory?.sessions??[],bars=(s.chartHistory?.minutes??[]).filter(b=>b.timestampMs+60000<=now&&sessions.some(v=>b.timestampMs>=v.openMs&&b.timestampMs<v.closeMs&&(b.timestampMs-v.openMs)%60000===0)).sort((a,b)=>a.timestampMs-b.timestampMs);
 const last=bars.at(-1),today=new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
 if(!last||now-last.timestampMs-60000>180000||new Date(last.timestampMs).toLocaleDateString('en-CA',{timeZone:'America/New_York'})!==today||bars.length<201)return empty;
 if(bars.some((b,i)=>![b.open,b.high,b.low,b.close,b.volume].every(Number.isFinite)||b.low<=0||b.volume<0||b.high<Math.max(b.open,b.close)||b.low>Math.min(b.open,b.close)||i>0&&b.timestampMs===bars[i-1].timestampMs))return empty;
 const recent=bars.slice(-201);if(recent.some((b,i)=>i>0&&sessions.some(v=>b.timestampMs>=v.openMs&&b.timestampMs<v.closeMs&&recent[i-1].timestampMs>=v.openMs)&&b.timestampMs-recent[i-1].timestampMs!==60000))return empty;
 const ema=(period:number,end=bars.length)=>{let value=bars.slice(0,period).reduce((z,b)=>z+b.close,0)/period;for(let i=period;i<end;i++)value+=(bars[i].close-value)*2/(period+1);return value;};
 const e9=ema(9),e20=ema(20),e50=ema(50),e200=ema(200),p9=ema(9,bars.length-1),p20=ema(20,bars.length-1),prior=bars.at(-2)!;
 const atr=bars.slice(-15).slice(1).reduce((z,b,i)=>{const p=bars.slice(-15)[i];return z+Math.max(b.high-b.low,Math.abs(b.high-p.close),Math.abs(b.low-p.close));},0)/14,tol=atr*.05;
 const rebound=(e:number,p:number)=>[1,-1].find(sign=>(p9-p20)*sign>0&&(e9-e20)*sign>0&&(prior.close-p)*sign>tol&&(last.close-e)*sign>tol&&(last.close-prior.close)*sign>0&&(sign>0?prior.low<=p+tol:prior.high>=p-tol))??0;
 const cross=p9<=p20&&e9>e20?1:p9>=p20&&e9<e20?-1:0;
 return {available:true,ema9:e9,ema20:e20,ema50:e50,ema200:e200,cross,rebound9:rebound(e9,p9),rebound20:rebound(e20,p20),label:`EMA 1m · 9 $${e9.toFixed(2)} · 20 $${e20.toFixed(2)} · 50 $${e50.toFixed(2)} · 200 $${e200.toFixed(2)} · ${e9>e20?'9 sobre 20':e9<e20?'9 bajo 20':'9 = 20'}${cross?' · cruce '+(cross>0?'alcista':'bajista'):''} · rebotes observados; no confirman una entrada`};
}
