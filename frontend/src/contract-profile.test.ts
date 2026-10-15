import {it,expect} from 'vitest';
import type {LiveState} from './live';
import type {EtfReport} from './etf-dealers';
import type {EtfDetail} from './etf-enrichment';
import {expiryReport} from './etf-enrichment';
import {gammaLiveModel,gammaContractRows} from './gamma-live-model';
import {scopeContractReport} from './contract-profile';
const now=Date.parse('2026-10-05T10:00:00-04:00'),today='2026-10-05',next='2026-10-06';
function fixture(symbol:string):LiveState {
 const details=[today,next].flatMap(expiration=>[100,101].flatMap(strike=>['call','put'].map(kind=>({ticker:`${symbol}:${expiration}:${strike}:${kind}`,expiration,strike,kind,oi:10,volume:null,dex:500,gex:2000,delta:kind==='call'?.5:-.5,gamma:.02} as EtfDetail))));
 const r={symbol,at:now,days:28,spot:100,priceAt:now,contracts:details.length,rows:[],expirations:[today,next],enrichment:{details,oiReferenceDate:null,previousObservedDate:null,oiComparable:false,priceFresh:true,oiKnown:details.length,gammaKnown:details.length,deltaKnown:details.length,quotesFresh:0,storageError:null,model:'Test fixture'},gammaComplete:true,deltaComplete:true} as EtfReport;
 return {symbol,publishedMs:now,optionSnapshotStatus:{phase:'ready',observedMs:now,structuralEnrichment:r,horizons:[{mode:'DAY',observedMs:now,expirations:[today],structure:{complete:false,gexAvailable:false,selected:{},routes:[]},exposure:{gexComplete:false}}]}} as unknown as LiveState;
}
it.each(['SPY','QQQ'])('chart and scanner use identical OI/GEX for %s and the same expiration',symbol=>{
 const s=fixture(symbol),r=s.optionSnapshotStatus.structuralEnrichment!,model=gammaLiveModel(s,symbol,true,now),scanner=expiryReport(r,today)!;
 expect(model.profileGexRows).toEqual([]);expect(model.contractProfile!.rows).toEqual(scanner.rows);
 for(const metric of ['oi','gex'] as const)expect(gammaContractRows(model,metric)).toEqual(scanner.rows.map(row=>({strike:row.strike,call:metric==='oi'?row.call.oi:row.call.gamma,put:metric==='oi'?row.put.oi:row.put.gamma})));
 expect(gammaContractRows(model,'gex')[0].call).toBe(2000);expect(model.rows).toEqual([]);expect(model.levels.every(l=>!l.dominant)).toBe(true);
});
it('preserves known-side bars when the other side is unknown, with OI independent of gamma',()=>{
 const s=fixture('QQQ');s.optionSnapshotStatus.structuralEnrichment!.enrichment!.details[0].gex=null;
 const m=gammaLiveModel(s,'QQQ',true,now);expect(gammaContractRows(m,'gex')[0]).toEqual({strike:100,call:null,put:2000});expect(gammaContractRows(m,'oi')[0].call).toBe(10);
});
it('uses actual next-session contracts without borrowing the current or four-week universe',()=>{
 const s=fixture('SPY'),h=s.optionSnapshotStatus.horizons[0];h.mode='NEXT_SESSION';h.expirations=[next];
 const r=s.optionSnapshotStatus.structuralEnrichment!;r.enrichment!.details.filter(c=>c.expiration===next).forEach(c=>c.oi=30);
 const m=gammaLiveModel(s,'SPY',true,now);expect(m.contractProfile!.expirations).toEqual([next]);expect(gammaContractRows(m,'oi')[0].call).toBe(30);expect(m.complete).toBe(false);
 expect(scopeContractReport(r,[today,next]).rows[0].call.oi).toBe(40);
});
it.each(['foreign','stale','future','mismatch','duplicate','negative','inactive'])('rejects %s without borrowing old structural bars',kind=>{
 const s=fixture('QQQ'),r=s.optionSnapshotStatus.structuralEnrichment!;
 if(kind==='foreign')r.symbol='SPY';if(kind==='future')r.at++;if(kind==='mismatch')r.at--;
 if(kind==='duplicate')r.enrichment!.details.push(r.enrichment!.details[0]);if(kind==='negative')r.enrichment!.details[0].gex=-1;
 const m=gammaLiveModel(s,'QQQ',kind!=='inactive',kind==='stale'?now+300001:now);expect(gammaContractRows(m,'gex')).toEqual([]);expect(gammaContractRows(m,'oi')).toEqual([]);
});
