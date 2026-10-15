import type {AuctionReading} from './auction-map-panel';
const day=(ms:number)=>new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'}).format(ms);
/** Display-only memory. Never feeds signals, bots or the native accumulation engine. */
class AuctionDisplayMemory {
 private rows=new Map<string,AuctionReading>();
 clear(){this.rows.clear();}
 read(symbol:string,now:number){for(const [key,row] of this.rows)if(row.asOfMs>now||day(row.asOfMs)!==day(now))this.rows.delete(key);return this.rows.get(symbol);}
 remember(r:AuctionReading){const old=this.rows.get(r.symbol);if(!old||r.asOfMs>old.asOfMs)this.rows.set(r.symbol,structuredClone(r));}
}
export const auctionDisplayMemory=new AuctionDisplayMemory();
export const auctionDisplayDay=day;
