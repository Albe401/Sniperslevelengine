import type {BlockReport,BlockPeriod} from './block-pressure';
import type {LiveState} from './live';
const day=(now:number)=>new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York',year:'numeric',month:'2-digit',day:'2-digit'}).format(now);
const prefix='snipers-blocks-v1:';
export function validBlockPeriod(p:BlockPeriod|undefined):p is BlockPeriod{return !!p&&Number.isInteger(p.count)&&p.count>=0&&[p.total,p.buy,p.sell,p.unknown,p.through_ms].every(n=>Number.isFinite(n)&&n>=0)&&Math.abs(p.total-p.buy-p.sell-p.unknown)<=Math.max(.01,p.total*1e-8)&&Array.isArray(p.top_levels)&&p.top_levels.length<=20&&p.top_levels.every(z=>Number.isFinite(z.price)&&z.price>0&&[z.total,z.buy,z.sell,z.count].every(n=>Number.isFinite(n)&&n>=0));}
export class BlockMemory {
 private values=new Map<string,BlockReport>();private writes=new Map<string,number>();
 constructor(private storage?:Pick<Storage,'getItem'|'setItem'>){}
 get(symbol:string,now:number):BlockReport|undefined{
  if(!/^[A-Z][A-Z0-9.]{0,14}$/.test(symbol))return;
  let r=this.values.get(symbol);
  if(!r)try{const raw=this.storage?.getItem(prefix+symbol);if(raw&&raw.length<100000)r=JSON.parse(raw);}catch{}
  if(!r||r.symbol!==symbol||r.date!==day(now)||!validBlockPeriod(r.premarket)||!validBlockPeriod(r.regular)||r.premarket.through_ms>now||r.regular.through_ms>now)return;
  this.values.set(symbol,r);return r;
 }
 remember(s:LiveState|undefined,now=Date.now(),force=false){
  const r=s?.offExchange?.analytics?.blocks;
  if(!r||r.symbol!==s?.symbol||!Number.isFinite(s.publishedMs)||s.publishedMs>now||now-s.publishedMs>3000||r.date!==day(now)||!validBlockPeriod(r.premarket)||!validBlockPeriod(r.regular)||r.premarket.through_ms>now||r.regular.through_ms>now)return;
  const old=this.get(r.symbol,now),ready=s.offExchange.history?.[r.date]?.phase==='ready';
  const recovering=!ready||!s.offExchange.blockQuoteStatus?.startsWith('Cotizaciones históricas consultadas');
  const choose=(p:BlockPeriod,previous:BlockPeriod|undefined)=>{
   if(!previous)return p;
   if(p.through_ms<previous.through_ms)return previous;
   // Reloading the tape and recovering its quotes are independent steps.
   // A complete tape can still temporarily lose all classified amounts.
   const classified=p.buy+p.sell,priorClassified=previous.buy+previous.sell;
   if(recovering&&(p.total<previous.total||classified<priorClassified))return previous;
   return p;
  };
  const saved={...r,premarket:choose(r.premarket,old?.premarket),regular:choose(r.regular,old?.regular)};
  const copy=JSON.parse(JSON.stringify(saved)) as BlockReport;
  if(this.values.size>=200&&!this.values.has(r.symbol))this.values.delete(this.values.keys().next().value!);
  this.values.set(r.symbol,copy);
  const changed=!old||(['premarket','regular'] as const).some(k=>['count','total','buy','sell','unknown','complete','finalized'].some(field=>old[k][field as keyof BlockPeriod]!==copy[k][field as keyof BlockPeriod]));
  if(force||changed||now-(this.writes.get(r.symbol)??0)>=5000)try{this.storage?.setItem(prefix+r.symbol,JSON.stringify(copy));this.writes.set(r.symbol,now);}catch{}
 }
 flush(){for(const [symbol,r] of this.values)try{this.storage?.setItem(prefix+symbol,JSON.stringify(r));}catch{}}
}
let storage:Storage|undefined;try{storage=globalThis.localStorage;}catch{}
export const blockMemory=new BlockMemory(storage);
if(typeof window!=='undefined'){
 window.addEventListener('pagehide',()=>blockMemory.flush());
 window.addEventListener('beforeunload',()=>blockMemory.flush());
 document.addEventListener('visibilitychange',()=>{if(document.visibilityState==='hidden')blockMemory.flush();});
}
