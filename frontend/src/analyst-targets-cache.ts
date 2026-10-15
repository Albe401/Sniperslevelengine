export interface TargetRow {symbol:string;low:number|null;average:number|null;high:number|null;status:string;fetchedMs:number}
interface StorageLike {getItem(key:string):string|null;setItem(key:string,value:string):void}
const key='snipers.fmp.targets.daily.v1';
export const targetDate=(ms:number)=>new Date(ms).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
function valid(r:TargetRow|undefined,symbol:string,now:number):r is TargetRow & {low:number;average:number;high:number} {
 return !!r&&r.symbol===symbol&&/^[A-Z][A-Z0-9.-]{0,14}$/.test(symbol)&&r.status==='Disponible'&&typeof r.fetchedMs==='number'&&Number.isFinite(r.fetchedMs)&&r.fetchedMs>=0&&r.fetchedMs<=now&&[r.low,r.average,r.high].every(n=>typeof n==='number'&&Number.isFinite(n)&&n>0)&&r.low!<=r.average!&&r.average!<=r.high!&&targetDate(r.fetchedMs)===targetDate(now);
}
/** Read-through display cache; the backend remains the authority for quota and vendor requests. */
export class DailyTargetsCache {
 private rows:Record<string,TargetRow>={};
 constructor(private storage?:StorageLike,now=Date.now()){
  try{const raw=storage?.getItem(key);if(raw&&raw.length<=200000){const saved=JSON.parse(raw);if(saved?.version===1&&saved.rows&&typeof saved.rows==='object'&&!Array.isArray(saved.rows))this.merge(saved.rows,now,false);}}catch{/* Keep rendering if local storage is unavailable/corrupt. */}
 }
 merge(rows:Record<string,TargetRow>,now=Date.now(),persist=true){
  for(const [symbol,row] of Object.entries(rows??{}).slice(0,500)){if(valid(row,symbol,now)&&(!valid(this.rows[symbol],symbol,now)||row.fetchedMs>=this.rows[symbol].fetchedMs))this.rows[symbol]={symbol,low:row.low,average:row.average,high:row.high,status:'Disponible',fetchedMs:row.fetchedMs};}
  for(const symbol of Object.keys(this.rows))if(!valid(this.rows[symbol],symbol,now))delete this.rows[symbol];
  if(persist)try{this.storage?.setItem(key,JSON.stringify({version:1,rows:this.rows}));}catch{/* In-memory cache still works. */}
 }
 get(symbol:string,now=Date.now()){const row=this.rows[symbol];return valid(row,symbol,now)?{...row}:undefined;}
}
