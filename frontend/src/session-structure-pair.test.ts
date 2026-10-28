import {describe,expect,it} from 'vitest';
import type {LiveState} from './live';
import {fiveMinuteStructure,readPriceStructureSeries,priceStructureBars,type PriceStructureCard,type FiveMinuteStructureCard} from './price-structure';
import {sessionStructurePair} from './session-structure-pair';
const now=Date.parse('2026-10-27T10:40:00-04:00');
function card(side:PriceStructureCard['side'],interval:5|15,endMs:number){return {interval,side,endMs,sourceSymbol:'QQQ',sourcePublishedMs:now,bars:120,atr:1,lastClose:720,seedDifference:.001,highs:[0,1].map(i=>({kind:'high',price:720+i,index:i+2,at:now-3000000+i*300000,confirmedMs:now-2000000+i*300000})),lows:[0,1].map(i=>({kind:'low',price:710+i,index:i+2,at:now-4000000+i*300000,confirmedMs:now-2500000+i*300000})),reference:false}as unknown as FiveMinuteStructureCard|PriceStructureCard;}
const reading={side:'CALL',availability:'current',closedAtMs:now-600000,livePriceReady:true,subtitle:'15m',reason:''}as const;
describe('causal session structure comparison, isolated from entry rules',()=>{
 it.each(['CALL','PUT']as const)('compares matching %s pivots at different valid closed buckets',side=>{
  const a=card(side,5,now)as FiveMinuteStructureCard,b=card(side,15,now-600000)as PriceStructureCard,before=JSON.stringify({a,b});const p=sessionStructurePair('QQQ',a,b,reading,now);
  expect(p.side).toBe(side);expect(p.availability).toBe('current');expect(p.entryAuthorized).toBe(false);expect(p.probability).toBeNull();expect(JSON.stringify({a,b})).toBe(before);
 });
 it('retains disagreement and historical references instead of inventing agreement',()=>{
  const a=card('PUT',5,now)as FiveMinuteStructureCard,b=card('CALL',15,now-600000)as PriceStructureCard;
  expect(sessionStructurePair('QQQ',a,b,reading,now).side).toBe('MIXTA');
  expect(sessionStructurePair('QQQ',a,b,{...reading,availability:'reference'},now).availability).toBe('reference');
 });
 it.each(['foreign-source','future-publication','future','older-than15','too-far-ahead','missing-pivots','seed','wrong15close','pending15']as const)('keeps %s partial',kind=>{
  const a=card('CALL',5,now)as FiveMinuteStructureCard,b=card('CALL',15,now-600000)as PriceStructureCard;
  if(kind==='foreign-source')a.sourceSymbol='SPY';if(kind==='future-publication')a.sourcePublishedMs=now+1;if(kind==='future')a.endMs=now+1;if(kind==='older-than15')a.endMs=b.endMs!-1;if(kind==='too-far-ahead')b.endMs=now-900000;
  if(kind==='missing-pivots')a.highs=[];if(kind==='seed')a.seedDifference=.1;
  const r=kind==='wrong15close'?{...reading,closedAtMs:now-1}:kind==='pending15'?{...reading,availability:'pending'as const}:reading;
  expect(sessionStructurePair('QQQ',a,b,r,now).side).toBe('PARCIAL');
 });
 it('uses only fully closed RTH5m buckets and rejects a missing latest minute',()=>{
  const open=now-4200000,session={date:'2026-10-27',openMs:open,closeMs:open+23400000},history={sessions:[session],minutes:Array.from({length:70},(_,i)=>({timestampMs:open+i*60000,open:100,high:101,low:99,close:100,volume:10}))};
  expect(priceStructureBars(history,5,now-1).bars).toHaveLength(13);expect(priceStructureBars(history,5,now).bars).toHaveLength(14);
  const s={symbol:'QQQ',publishedMs:now,chartHistory:history}as unknown as LiveState;
  expect(fiveMinuteStructure(s,'OTHER',true,now).side).toBe('SIN DATOS');expect(fiveMinuteStructure(s,'QQQ',false,now).side).toBe('SIN DATOS');
  history.minutes.pop();expect(fiveMinuteStructure(s,'QQQ',true,now).side).toBe('SIN DATOS');
  expect(readPriceStructureSeries({bars:[],gaps:0,reason:'none'},5,now).reason).toBe('none');
 });
});
