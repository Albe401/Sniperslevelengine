import type {BidAskWindow} from './strike-bid-ask';
import {sameStrike} from './strike-key';
/** Exact-strike sampled five-minute windows. CALL/PUT is not buy/sell. */
export function barrierFlow(rows:BidAskWindow[]|undefined,strike:number,expirations:string[],fresh:boolean){
 if(!fresh||!expirations.length)return null;
 const selected=(rows??[]).filter(r=>sameStrike(r.strike,strike)&&expirations.includes(r.expiration)&&['call','put'].includes(r.kind));
 if(new Set(selected.map(r=>r.ticker)).size!==selected.length)return null;
 for(const expiration of new Set(expirations))for(const kind of ['call','put'])if(!selected.some(r=>r.expiration===expiration&&r.kind===kind))return null;
 if(selected.some(r=>!r.covered||[r.bid,r.ask,r.unknown].some(v=>v==null||!Number.isFinite(v)||v<0)))return null;
 const sum=(kind:string)=>selected.filter(r=>r.kind===kind).reduce((n,r)=>n+r.ask!+r.bid!+r.unknown!,0);
 const call=sum('call'),put=sum('put'),total=call+put;
 const buy=selected.reduce((n,r)=>n+r.ask!,0),sell=selected.reduce((n,r)=>n+r.bid!,0),known=buy+sell;
 if(![total,buy,sell,known].every(Number.isFinite)||total<=0)return null;
 return {call,put,total,buy,sell,unknown:total-known,callPct:100*call/total,putPct:100*put/total,buyPct:known>0?100*buy/known:null,sellPct:known>0?100*sell/known:null,coverage:100*known/total};
}
