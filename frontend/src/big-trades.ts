import type {CvdRead,BigTrade} from './cvd';
const dateET=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York',year:'numeric',month:'2-digit',day:'2-digit'});
/** Individual executed stock prints; side is inferred from contemporaneous quotes. */
export function chartBigTrades(read:CvdRead|undefined,symbol:string,minimum:number,now:number):BigTrade[]{
 if(!Number.isFinite(now)||!read||read.symbol!==symbol||read.date!==dateET.format(now)||read.at>now||now-read.at>3000||!Number.isFinite(minimum)||minimum<100000)return [];
 const seen=new Set<number>();
 return (read.bigTrades??[]).filter(p=>{
  if(![p.at,p.price,p.shares,p.notional].every(Number.isFinite)||p.price<=0||p.shares<=0||p.notional<minimum||Math.abs(p.price*p.shares-p.notional)>Math.max(.01,p.notional*1e-9)||![-1,0,1].includes(p.side)||!Number.isSafeInteger(p.sequence)||p.sequence<0||seen.has(p.sequence)||p.at>now+1000||Math.abs(p.at-now)>86400000||dateET.format(p.at)!==read.date)return false;
  seen.add(p.sequence);return true;
 }).slice(-1000);
}
