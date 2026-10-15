import type {LiveState} from './live';
type Point={at:number;value:number;segment:number};
/** Observed stock CVD only. Never manufactures historical delta from candles. */
export class CvdChartEngine {
 private key='';private points:Point[]=[];private segment=0;private broken=false;private gaps=0;private since:number|null|undefined;
 observe(s:LiveState|undefined,symbol:string,active:boolean,now:number){
  const c=s?.cvd,period=s?.metrics.phase==='Premarket'?'premarket':'regular',p=c?.[period];
  const key=c?`${symbol}:${c.date}:${period}`:this.key.startsWith(symbol+':')?this.key:`${symbol}::${period}`;
  if(key!==this.key){this.key=key;this.points=[];this.segment=0;this.broken=false;this.gaps=0;this.since=undefined;}
  const valid=Number.isFinite(now)&&active&&s?.symbol===symbol&&s.stocks.phase==='streaming'&&['Sesión RTH','Premarket'].includes(s.metrics.phase)&&c?.symbol===symbol&&c.available&&c.date===new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'})&&c.at<=now&&now-c.at<=3000&&p!=null&&p.lastMs!=null&&p.lastMs<=now+1000&&now-p.lastMs<=15000&&p.sinceMs!=null&&Number.isFinite(p.sinceMs)&&p.sinceMs<=p.lastMs&&Number.isInteger(p.gaps)&&p.gaps>=0&&[p.buy,p.sell,p.unknown,p.delta].every(Number.isFinite)&&p.buy>=0&&p.sell>=0&&p.unknown>=0&&Math.abs(p.delta-p.buy+p.sell)<1e-6;
  if(!valid){this.broken=true;return;}
  const last=this.points.at(-1);if(last&&c.at<last.at)return;
  if(last&&(this.broken||p.gaps!==this.gaps||p.sinceMs!==this.since||c.at-last.at>15000))this.segment++;
  this.broken=false;this.gaps=p.gaps;this.since=p.sinceMs;
  const history=p.history;
  // Accept only a complete, ordered curve matching this verified segment.
  if(history?.length&&history.length<=1200&&history.every((h,i)=>Number.isFinite(h.at)&&Number.isFinite(h.value)&&h.at>=p.sinceMs!&&h.at<=p.lastMs!&&(i===0||h.at>history[i-1].at))&&Math.abs(history.at(-1)!.value-p.delta)<1e-6){
   this.points=history.map(h=>({...h,segment:this.segment}));
   return;
  }
  const point={at:c.at,value:p.delta,segment:this.segment};
  if(last&&Math.floor(last.at/60000)===Math.floor(c.at/60000)&&last.segment===point.segment)this.points[this.points.length-1]=point;else this.points.push(point);
  if(this.points.length>2000)this.points.splice(0,this.points.length-2000);
 }
 series(bars:{timestampMs:number}[],interval:number){
  let index=0;let previousSegment:number|undefined;
  return bars.map((bar,i)=>{
   const start=interval===1440?Date.parse(new Date(bar.timestampMs).toISOString().slice(0,10)+'T00:00:00Z'):bar.timestampMs;
   const end=interval===1440?start+86400000:Math.min(start+interval*60000,bars[i+1]?.timestampMs??Infinity);
   while(index<this.points.length&&this.points[index].at<start)index++;
   let point:Point|undefined;while(index<this.points.length&&this.points[index].at<end)point=this.points[index++];
   const time=bar.timestampMs/1000;
   if(!point)return {time};
   if(previousSegment!=null&&previousSegment!==point.segment){previousSegment=point.segment;return {time};}
   previousSegment=point.segment;return {time,value:point.value};
  });
 }
}
export const cvdChartEngine=new CvdChartEngine();
