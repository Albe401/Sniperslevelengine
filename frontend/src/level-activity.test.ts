import {describe,it,expect} from 'vitest';
import {readLevelActivities,type LevelActivityInput} from './level-activity';
import type {CvdRead} from './cvd';
import type {AuctionReading} from './auction-map-panel';
import type {BidAskWindow} from './strike-bid-ask';
import {strikeKey} from './strike-key';

const now=Date.parse('2026-10-26T14:00:00Z'),date='2026-10-26';
const zone=(price:number,sell:number,unknown=20)=>({lower:price,upper:price+.01,firstMs:now-50000,lastMs:now-1000,shares:sell+10+unknown,buy:10,sell,unknown});
function input():LevelActivityInput {
 const cvd={symbol:'QQQ',date,at:now,available:true,regular:{lastMs:now-1000},executionEvidence:{version:'execution-evidence-v1',status:'observing',observedSinceMs:now-120000,windows:[{seconds:60,startMs:now-60000,endMs:now,complete:true,classifiedPct:30,excludedZones:2,zones:[zone(100,200),zone(101,100)]}]}} as unknown as CvdRead;
 const auction={version:'auction-map-ticks-v1',symbol:'QQQ',asOfMs:now,provisional:true,acceptedTrades:10,totalVolume:1000,pocLow:100,pocHigh:101,val:100,vah:102,bins:[{low:100,high:101,volume:500,brackets:2,volumeRatio:.5,timeRatio:.5,kind:'acceptance'},{low:101,high:102,volume:500,brackets:3,volumeRatio:1,timeRatio:.81,kind:'acceptance'}],routes:[],reaction:'pending',calibrated:false} as AuctionReading;
 const rows=(strike:number,bid:number):BidAskWindow[]=>['call','put'].map(kind=>({ticker:kind+strike,strike,kind,expiration:date,covered:true,bid,ask:10,unknown:30}));
 return {symbol:'QQQ',active:true,publishedMs:now,cvd,auction,optionRows:[...rows(100,5),...rows(101,50)],expirations:[date],optionFresh:true,optionAtMs:now};
}
const read=(i=input(),at=now,prices=[100,101])=>readLevelActivities(prices,i,at);
const record=(r:ReturnType<typeof read>,price:number)=>r.get(strikeKey(price)!)!;
const rawWindow=(i:LevelActivityInput)=>(i.cvd!.executionEvidence!.windows[0] as unknown as {complete:boolean;startMs:number;endMs:number;zones:ReturnType<typeof zone>[]});

describe('level map source activities',()=>{
 it('keeps three source units independent and reuses acceptance score',()=>{
  const r=read(),a=record(r,100),b=record(r,101);
  expect(a.stockSales.value).toBe(200);expect(a.stockSales.max).toBe(true);
  expect(b.optionSales.value).toBe(100);expect(b.optionSales.max).toBe(true);
  expect(a.optionSales.max).toBe(false);expect(b.stockSales.max).toBe(false);
  expect(a.acceptance.value).toBe(.5);expect(b.acceptance.value).toBe(.9);
  expect(b.acceptance.max).toBe(true);expect(a.stockSales.detail).toContain('acciones');expect(b.optionSales.detail).toContain('contratos');
 });
 it('retains unknown and low coverage rather than applying absorption filters',()=>{
  const r=record(read(),100);
  expect(r.stockSales.value).toBe(200);expect(r.stockSales.detail).toContain('30.0%');expect(r.stockSales.detail).toContain('Sin clasificar 20');expect(r.stockSales.detail).toContain('zonas excluidas 2');
  expect(r.optionSales.value).toBe(10);expect(r.optionSales.detail).toContain('Sin clasificar 60');
 });
 it.each([3001,16000])('rejects stale publication at age %i',age=>{
  const i=input();i.publishedMs=now-age;const r=record(read(i),100);
  expect([r.acceptance.value,r.stockSales.value,r.optionSales.value]).toEqual([null,null,null]);
 });
 it('rejects foreign CVD and auction independently of options',()=>{
  const i=input();i.cvd!.symbol='SPY';i.auction!.symbol='SPY';const r=record(read(i),100);
  expect(r.stockSales.value).toBeNull();expect(r.acceptance.value).toBeNull();expect(r.optionSales.value).toBe(10);
 });
 it('turns AMT off after source trades stop even if auction asOf refreshes',()=>{
  const i=input();i.cvd!.regular.lastMs=now-15001;
  const r=record(read(i),100);expect(r.acceptance.value).toBeNull();expect(r.stockSales.value).toBeNull();
 });
 it('rejects future/wrong-day/unavailable stock evidence',()=>{
  for(const mutate of [(i:LevelActivityInput)=>i.cvd!.at=now+1,(i:LevelActivityInput)=>i.cvd!.date='2026-10-23',(i:LevelActivityInput)=>i.cvd!.available=false]){
   const i=input();mutate(i);const r=record(read(i),100);expect(r.stockSales.value).toBeNull();expect(r.acceptance.value).toBeNull();
  }
 });
 it('requires one complete exact 60-second window observed from its start',()=>{
  for(const mutate of [(i:LevelActivityInput)=>rawWindow(i).complete=false,(i:LevelActivityInput)=>rawWindow(i).startMs++, (i:LevelActivityInput)=>i.cvd!.executionEvidence!.observedSinceMs=now-59999]){
   const i=input();mutate(i);expect(record(read(i),100).stockSales.value).toBeNull();
  }
  const i=input();i.cvd!.executionEvidence!.windows.push(i.cvd!.executionEvidence!.windows[0]);expect(record(read(i),100).stockSales.value).toBeNull();
 });
 it('does not borrow the nearest price bucket or turn missing zones into zero',()=>{
  const r=read(input(),now,[100.005,100.01,100.02]);
  expect(record(r,100.005).stockSales.value).toBe(200);expect(record(r,100.01).stockSales.value).toBeNull();expect(record(r,100.02).stockSales.value).toBeNull();
 });
 it('neutralizes duplicate/overlapping or malformed stock price ranges',()=>{
  for(const replacement of [[zone(100,200),zone(100,10)],[zone(100,200),zone(100.005,10)],[zone(100.005,10)],[{...zone(100,200),sell:-1}]]){
   const i=input();rawWindow(i).zones=replacement;expect(record(read(i),100).stockSales.value).toBeNull();
  }
 });
 it('does not throw on malformed option rows or invalid evaluator clocks',()=>{
  const i=input();i.optionRows=[null] as unknown as BidAskWindow[];
  expect(record(read(i),100).optionSales.value).toBeNull();
  for(const at of [NaN,Infinity,-1])for(const x of read(input(),at).values())expect(x.acceptance.value).toBeNull();
 });
 it('neutralizes overlapping auction bins and rejects stale/bar-derived profiles',()=>{
  const i=input();i.auction!.bins[1].low=100.5;expect(record(read(i),100).acceptance.value).toBeNull();
  for(const mutate of [(i:LevelActivityInput)=>i.auction!.asOfMs=now-15001,(i:LevelActivityInput)=>i.auction!.version='auction-map-bars-reference-v1']){
   const j=input();mutate(j);expect(record(read(j),100).acceptance.value).toBeNull();
  }
 });
 it('requires current complete option scope with both CALL and PUT, not PUT-as-sales',()=>{
  const i=input();i.optionRows=i.optionRows!.filter(row=>!(row.strike===100&&row.kind==='put'));expect(record(read(i),100).optionSales.value).toBeNull();
  const j=input();j.optionRows!.find(row=>row.strike===100&&row.kind==='put')!.ask=50000;
  expect(record(read(j),100).optionSales.value).toBe(10);
  j.expirations.push('2026-10-27');expect(record(read(j),100).optionSales.value).toBeNull();
 });
 it('rejects stale/future option timestamp, past scope and incomplete sampled rows',()=>{
  for(const mutate of [(i:LevelActivityInput)=>i.optionAtMs=now-3001,(i:LevelActivityInput)=>i.optionAtMs=now+1,(i:LevelActivityInput)=>i.optionFresh=false,(i:LevelActivityInput)=>i.expirations=['2026-10-23'],(i:LevelActivityInput)=>i.optionRows![0].covered=false]){
   const i=input();mutate(i);expect(record(read(i),100).optionSales.value).toBeNull();
  }
 });
 it('requires all selected expirations and rejects duplicate option series',()=>{
  const i=input();i.expirations.push('2026-10-27');i.optionRows!.push(...i.optionRows!.map(row=>({...row,ticker:row.ticker+'next',expiration:'2026-10-27'})));
  expect(record(read(i),100).optionSales.value).toBe(20);
  i.optionRows!.push({...i.optionRows![0]});expect(record(read(i),100).optionSales.value).toBeNull();
 });
 it('marks every positive tie and never marks a zero as maximum',()=>{
  const i=input();rawWindow(i).zones[1].sell=200;rawWindow(i).zones[1].shares=230;
  const r=read(i);for(const x of r.values()){expect(x.stockSales.max).toBe(true);expect(x.stockSales.tied).toBe(true);expect(x.stockSales.intensity).toBe(1);}
  const j=input();for(const z of rawWindow(j).zones){z.sell=0;z.shares=z.buy+z.unknown;}
  for(const x of read(j).values()){expect(x.stockSales.value).toBe(0);expect(x.stockSales.max).toBe(false);expect(x.stockSales.intensity).toBe(0);}
 });
 it('ranks only mapped records and deduplicates normalized prices',()=>{
  const r=read(input(),now,[100,100.000000000001,NaN,-1]);expect(r.size).toBe(1);
  expect(record(r,100).optionSales.max).toBe(true);expect(record(r,100).optionSales.intensity).toBe(1);
 });
 it('supports explicitly identified captures without calling them current',()=>{
  const i=input();i.active=false;i.captured=true;const r=record(read(i),100);
  expect(r.stockSales.value).toBe(200);expect(r.captured).toBe(true);expect(r.acceptance.label).toBe('Baja');expect(r.acceptance.detail).toContain('Captura');expect(r.stockSales.detail).toContain('no actividad actual');
  i.captured=false;expect(record(read(i),100).stockSales.value).toBeNull();
 });
 it('does not mutate frozen inputs and returns finite bounded style values',()=>{
  const i=input();const before=JSON.stringify(i);Object.freeze(i.expirations);Object.freeze(i.optionRows);Object.freeze(i.auction!.bins);Object.freeze(rawWindow(i).zones);Object.freeze(i);
  const r=read(i);expect(JSON.stringify(i)).toBe(before);
  for(const x of r.values())for(const field of ['acceptance','stockSales','optionSales'] as const){expect(Number.isFinite(x[field].intensity)).toBe(true);expect(x[field].intensity).toBeGreaterThanOrEqual(0);expect(x[field].intensity).toBeLessThanOrEqual(1);}
 });
});
