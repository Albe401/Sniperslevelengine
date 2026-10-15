import {it,expect} from 'vitest';
import {expiryReport,rankStrikes,etfFlowPanel,type EtfDetail,type EtfFlow} from './etf-enrichment';
import type {EtfReport} from './etf-dealers';
const detail=(expiration:string,kind:string,oi:number|null):EtfDetail=>({ticker:expiration+kind,expiration,kind,strike:500,oi,oiChange:null,volume:3,volumeOi:null,delta:.5,gamma:.01,dex:100,gex:200,iv:.2,theta:-.1,vega:.1,charm:null,vanna:null,bid:1,ask:1.1,spreadPct:10,quoteAt:1000,realtime:true,fresh:true});
const r:EtfReport={symbol:'SPY',days:28,at:1000,spot:500,priceAt:1000,expirations:['2026-10-05','2026-10-09'],contracts:2,gammaComplete:true,deltaComplete:true,rows:[],enrichment:{oiReferenceDate:null,previousObservedDate:null,oiComparable:false,priceFresh:true,oiKnown:1,gammaKnown:2,deltaKnown:2,quotesFresh:2,storageError:null,model:'test',details:[detail('2026-10-05','call',10),detail('2026-10-09','put',null)]}};
it('isolates expiry profiles without mixing another expiration or turning unknown into zero',()=>{
 const one=expiryReport(r,'2026-10-05')!;expect(one.rows[0].call.oi).toBe(10);expect(one.rows[0].put.oi).toBe(0);expect(one.contracts).toBe(1);
 const two=expiryReport(r,'2026-10-09')!;expect(two.rows[0].put.oi).toBeNull();expect(rankStrikes(two,'oi')).toEqual([]);expect(rankStrikes(one,'oi')[0].value).toBe(10);
});
const flow:EtfFlow={symbol:'SPY',at:1000,connected:true,tracked:1,subscribed:1,lastEventMs:1000,flow:{contract_windows:[{ticker:'C',minutes:1,covered:true}],windows:[{minutes:1,event_count:1,contracts:5,premium:500,premium_complete:true,buckets:{'CALL BUY':{contracts:3,premium:300,trades:1}}}]}};
it('rejects foreign stale disconnected and unsubscribed options flow, preserves unknown and incomplete windows',()=>{
 expect(etfFlowPanel(flow,'SPY',true,1000)).toContain('<td>3</td>');expect(etfFlowPanel(flow,'SPY',true,1000)).toContain('<td>2</td>');
 for(const data of [{...flow,symbol:'QQQ'},{...flow,connected:false},{...flow,subscribed:0},{...flow,at:1001}])expect(etfFlowPanel(data,'SPY',true,1000)).not.toContain('<td>3</td>');
 expect(etfFlowPanel(flow,'SPY',true,17000)).not.toContain('<td>3</td>');expect(etfFlowPanel(flow,'SPY',true,1000)).toContain('5m · incompleto');
});
