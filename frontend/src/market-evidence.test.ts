import {expect,it} from 'vitest';
import type {LiveState} from './live';
import type {EtfDetail} from './etf-enrichment';
import {readMarketEvidence} from './market-evidence';
import {dailyEvidenceContracts} from './level-evidence';
import {buildIntradayControl,ControlTracker} from './intraday-control';
import {readDirectionScore} from './direction-scanner';
import {controlOutlook} from './control-outlook';
import {readRouteLive} from './route-live';
import {gammaReading} from './gamma-reading';
import {optionsCvdPanel} from './options-cvd';
import {intradayPressure} from './volatility-reading';
const start=Date.parse('2026-10-05T09:30:00-04:00'),now=start+30*60000,date='2026-10-05';
const contract=(strike:number,kind:string):EtfDetail=>({ticker:`${strike}${kind}`,strike,kind,expiration:date,oi:100,oiChange:5,volume:20,volumeOi:.2,delta:kind==='call'?.5:-.5,gamma:.01,dex:5000,gex:1000,iv:.2,theta:-.1,vega:.1,charm:.01,vanna:.02,bid:1,ask:1.1,spreadPct:10,quoteAt:now,fresh:true,realtime:true});
export function marketFixture():LiveState {
 const details=[99,101,102].flatMap(k=>['call','put'].map(t=>contract(k,t)));
 const buckets=Object.fromEntries(['CALL BUY','PUT SELL','PUT BUY','CALL SELL','UNKNOWN'].map((k,i)=>[k,{premium:[800,100,50,50,0][i],contracts:10,trades:2}]));
 const report={symbol:'QQQ',days:28,at:now,spot:100.6,priceAt:now,expirations:[date],contracts:details.length,rows:[],gammaComplete:true,deltaComplete:true,enrichment:{details,oiReferenceDate:null,previousObservedDate:'2026-10-02',oiComparable:true,priceFresh:true,oiKnown:6,gammaKnown:6,deltaKnown:6,quotesFresh:6,storageError:null,model:'BSM'}};
 return {symbol:'QQQ',publishedMs:now,startedMs:start,stocks:{phase:'streaming'},options:{phase:'streaming',lastEventMs:now},ticks:{QQQ:{symbol:'QQQ',price:100.6,tradeMs:now}},metrics:{session:date,phase:'Sesión RTH',asofMs:now,vwap:100.2},flow:{asof_ms:now,intraday_windows:[1,5].map(minutes=>({minutes,event_count:10,contracts:50,premium:1000,premium_complete:true,bullish_pct:90,bearish_pct:10,direction:'BULLISH FLOW',buckets})),contract_windows:[],windows:[]},chartHistory:{sessions:[{date,openMs:start,closeMs:start+390*60000}],minutes:Array.from({length:30},(_,i)=>({timestampMs:start+i*60000,open:100+i*.02,close:100.02+i*.02,high:100.04+i*.02,low:99.98+i*.02,volume:100,vwap:100.2}))},optionSnapshotStatus:{phase:'ready',observedMs:now,structuralEnrichment:report,horizons:[{mode:'DAY',observedMs:now,expirations:[date],structure:{complete:true,rows:3,selected:{M1:101},callWall:{strike:102,score:60},putWall:{strike:99,score:90},routes:[]}}]},cvd:{symbol:'QQQ',date,at:now,available:true,rejected:0,regular:{buy:800,sell:100,unknown:100,delta:700,coveragePct:90,sinceMs:start,lastMs:now,gaps:0,changeOne:100,changeFive:700},recentFive:{buy:800,sell:100,unknown:100,delta:700,startMs:now-300000,endMs:now,complete:true}}} as unknown as LiveState;
}
it('control, confluence, route, levels and gamma consume the same daily evidence',()=>{
 const s=marketFixture(),common=readMarketEvidence(s,'QQQ',true,now),control=buildIntradayControl(s,'QQQ',true,now);
 expect(common.score.side).toBe('CALL');expect(common.support).toBe('aligned');
 expect(control.marketEvidence?.score).toEqual(common.score);
 expect(readDirectionScore(s,control,true,now).call).toBe(common.score.call);
 expect(controlOutlook(control).summary.direction).toBe(common.direction);
 const route=readRouteLive(s,'QQQ',true,now);expect(route.market.score).toEqual(common.score);
 expect(route.market.levels.map(l=>l.evidence)).toEqual(common.levels.map(l=>l.evidence));
 expect(gammaReading(s,'QQQ',true,now).reading.marketEvidence?.score).toEqual(common.score);
 expect(new ControlTracker().update(control,now).control).toBe('COMPRADORES');
 expect(intradayPressure(control).value).toBe((common.score.call!-50)/50);
 expect(optionsCvdPanel(s,'QQQ',true,now,undefined,common)).toContain('ALINEADOS · LECTURA COMÚN');
});
it('counts exact daily contracts once and keeps concentration out of directional weights',()=>{
 const s=marketFixture(),a=readMarketEvidence(s,'QQQ',true,now),e=a.levels.find(l=>l.price===99)!.evidence!;
 expect(e.oi).toBe(200);expect(e.dex).toBe(10000);expect(e.gammaShare).toBeCloseTo(100/3);expect(e.oiChange).toBe(10);
 s.optionSnapshotStatus.structuralEnrichment!.enrichment!.details.forEach(d=>{d.oi!*=100;d.gex!*=100;d.dex!*=100;});
 expect(readMarketEvidence(s,'QQQ',true,now).score.call).toBe(a.score.call);
 s.optionSnapshotStatus.structuralEnrichment!.enrichment!.details[0].oi=null;
 const partial=readMarketEvidence(s,'QQQ',true,now).levels.find(l=>l.price===99)!.evidence!;
 expect(partial.oi).toBeNull();expect(partial.quality).toBe('partial');expect(partial.oiShare).toBeNull();
});
it.each(['foreign','future','stale','snapshot-mismatch','duplicate','expiry','negative'])('rejects unusable enriched evidence: %s',kind=>{
 const s=marketFixture(),r=s.optionSnapshotStatus.structuralEnrichment!;
 if(kind==='foreign')r.symbol='SPY';if(kind==='future')r.at=now+1;if(kind==='stale'){r.at=now-300001;s.optionSnapshotStatus.observedMs=r.at;s.optionSnapshotStatus.horizons[0].observedMs=r.at;}
 if(kind==='snapshot-mismatch')r.at--;if(kind==='duplicate')r.enrichment!.details.push(r.enrichment!.details[0]);if(kind==='expiry')s.optionSnapshotStatus.horizons[0].expirations=['2026-10-06'];if(kind==='negative')r.enrichment!.details[0].oi=-1;
 expect(dailyEvidenceContracts(s,'QQQ',true,now)).toBeUndefined();
});
it('uses recent CVD instead of accumulated CVD and rejects noncomparable windows',()=>{
 const s=marketFixture();s.cvd!.regular!.delta=-700;s.cvd!.regular!.buy=100;s.cvd!.regular!.sell=800;
 expect(readMarketEvidence(s,'QQQ',true,now).support).toBe('aligned');
 s.cvd!.recentFive!.buy=100;s.cvd!.recentFive!.sell=800;s.cvd!.recentFive!.delta=-700;
 const control=buildIntradayControl(s,'QQQ',true,now);expect(control.marketEvidence!.support).toBe('opposed');expect(readDirectionScore(s,control,true,now).cvd?.status).toBe('contradicts');
 s.cvd!.recentFive!.endMs-=4000;s.cvd!.recentFive!.startMs-=4000;expect(readMarketEvidence(s,'QQQ',true,now).support).toBe('pending');
});
it('cannot promote missing flow or a different expiration to a daily winner',()=>{
 const s=marketFixture();s.flow.intraday_windows![1].buckets!.UNKNOWN.premium=500;
 expect(readMarketEvidence(s,'QQQ',true,now).score.call).toBeNull();
 expect(readDirectionScore(s,buildIntradayControl(s,'QQQ',true,now),true,now).call).toBeNull();
 s.flowExpiry='2026-10-06';expect(readMarketEvidence(s,'QQQ',true,now).score.call).toBeNull();
});
it('chart and percentage ignore an independently overridden price component',()=>{
 const s=marketFixture(),r=buildIntradayControl(s,'QQQ',true,now);
 const component=r.components.find(c=>c.name==='PRECIO 2M + VWAP');if(component)component.signal=-1;
 const chart=readRouteLive(s,'QQQ',true,now);
 expect(readDirectionScore(s,r,true,now).call).toBe(chart.market.score.call);
 expect(r.marketEvidence?.key).toBe(chart.market.key);
});
it('revalidates a cached control reading after a disconnect or stale/future publication',()=>{
 const s=marketFixture(),r=buildIntradayControl(s,'QQQ',true,now);expect(r.marketEvidence!.score.call).not.toBeNull();
 s.options.phase='disconnected';expect(readDirectionScore(s,r,true,now).call).toBeNull();expect(readDirectionScore(s,r,true,now).cvd?.status).toBe('pending');
 s.options.phase='streaming';expect(readDirectionScore(s,r,true,now+4000).call).toBeNull();
 s.publishedMs=now+1;expect(readDirectionScore(s,r,true,now).call).toBeNull();
});
