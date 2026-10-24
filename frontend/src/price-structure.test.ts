import {describe,it,expect} from 'vitest';
import {classifyPriceStructure,priceStructureAtr,priceStructureBars,priceStructureCards,priceStructurePivots,type PriceBar} from './price-structure';
import type {ChartHistory} from './candle-series';
import type {LiveState} from './live';
const start=Date.parse('2026-10-05T09:30:00-04:00');
const rising=[100,101,103,106,104,102,104,107,110,108,105,107,110,113,111,108,110,114,117,115,112,114,116,115];
const bars=(values=rising):PriceBar[]=>values.map((close,i)=>({timestampMs:start+i*900000,endMs:start+(i+1)*900000,sessionDate:'2026-10-05',open:close-.1,high:close+.4,low:close-.4,close,volume:1000}));
const read=(b:PriceBar[])=>classifyPriceStructure({bars:b,gaps:0,reason:''},15,b.at(-1)!.endMs);
function history(count=390):ChartHistory {
 const session={date:'2026-10-05',openMs:start,closeMs:start+390*60000};
 return {sessions:[session],minutes:Array.from({length:count},(_,i)=>({timestampMs:start+i*60000,open:100+i*.01,high:101+i*.01,low:99+i*.01,close:100+i*.01,volume:100}))};
}
describe('closed-bar price structure',()=>{
 it('classifies higher highs and higher lows, and their symmetric falling case',()=>{
  const call=read(bars()),put=read(bars(rising.map(x=>220-x)));
  expect(call.side).toBe('CALL');expect(call.highChange).toBeGreaterThan(0);expect(call.lowChange).toBeGreaterThan(0);
  expect(put.side).toBe('PUT');expect(put.highChange).toBeLessThan(0);expect(put.lowChange).toBeLessThan(0);
  expect(call.atr).toBeCloseTo(put.atr!,10);
 });
 it('confirms extrema only after two subsequent bars close',()=>{
  const b=bars();const atPeak=priceStructurePivots(b.slice(0,19));
  expect(atPeak.some(p=>p.index===18)).toBe(false);
  const confirmed=priceStructurePivots(b.slice(0,21)).find(p=>p.index===18);
  expect(confirmed?.confirmedMs).toBe(b[20].endMs);
  expect(confirmed?.confirmedMs).toBeGreaterThan(confirmed!.at);
 });
 it('retains a broken structure as mixed even if price subsequently recovers',()=>{
  const b=bars().slice(0,23);b.push({...b.at(-1)!,timestampMs:b.at(-1)!.endMs,endMs:b.at(-1)!.endMs+900000,open:110,high:111,low:106,close:107});
  const broken=read(b);expect(broken.side).toBe('MIXTA');expect(broken.invalidated).toBe(true);
  b.push({...b.at(-1)!,timestampMs:b.at(-1)!.endMs,endMs:b.at(-1)!.endMs+900000,open:114,high:116,low:113,close:115});
  expect(read(b).side).toBe('MIXTA');
 });
 it('does not force a side from monotone prices without sufficient confirmed extrema',()=>{
  const r=read(bars(Array.from({length:30},(_,i)=>100+i)));
  expect(r.side).toBe('SIN DATOS');expect(r.reason).toContain('máximos');
 });
 it('rejects ambiguous outside-bar pivots and tied extrema',()=>{
  const b=bars([100,100,100,100,100]);b[2].high=110;b[2].low=90;
  expect(priceStructurePivots(b)).toEqual([]);
  expect(priceStructurePivots(bars([100,100,100,100,100]))).toEqual([]);
 });
 it('uses Wilder smoothing with a verifiable seed and includes overnight gaps',()=>{
  const b=bars(Array.from({length:16},()=>100));
  for(const row of b){row.open=100;row.high=101;row.low=99;row.close=100;}
  Object.assign(b[15],{open:110,high:111,low:109,close:110});
  expect(priceStructureAtr(b)).toBeCloseTo((2*13+11)/14,12);
  expect(priceStructureAtr(b.slice(0,14))).toBeNull();
 });
});
describe('canonical RTH aggregation and evidence guards',()=>{
 it('builds exactly 26 fifteen-minute, seven session-aligned hourly and one daily bars',()=>{
  const h=history(),now=start+390*60000;
  expect(priceStructureBars(h,15,now).bars).toHaveLength(26);
  expect(priceStructureBars(h,60,now).bars).toHaveLength(7);
  const daily=priceStructureBars(h,1440,now).bars;
  expect(daily).toHaveLength(1);expect(daily[0].endMs).toBe(now);expect(daily[0].volume).toBe(39000);
 });
 it('excludes a still-open bar and all future minutes',()=>{
  const h=history(),now=start+15*60000-1;
  expect(priceStructureBars(h,15,now).bars).toHaveLength(0);
  expect(priceStructureBars(h,1440,now).bars).toHaveLength(0);
 });
 it('uses the official early close for daily aggregation',()=>{
  const h=history(210);h.sessions[0].closeMs=start+210*60000;
  const r=priceStructureBars(h,1440,h.sessions[0].closeMs);
  expect(r.bars).toHaveLength(1);expect(r.bars[0].volume).toBe(21000);
 });
 it('never bridges a missing minute in the ATR/pivot sequence',()=>{
  const h=history();h.minutes=h.minutes.filter((_,i)=>i!==300);
  const r=priceStructureBars(h,15,start+390*60000);
  expect(r.gaps).toBe(1);expect(r.bars).toHaveLength(5);expect(r.bars[0].timestampMs).toBe(start+315*60000);
  expect(classifyPriceStructure(r,15,start+390*60000).side).toBe('SIN DATOS');
 });
 it('does not fall back to an older direction if the latest closed bucket is incomplete',()=>{
  const h=history(389),r=priceStructureBars(h,15,start+390*60000);
  expect(r.bars).toHaveLength(0);expect(r.reason).toContain('Faltan minutos');
 });
 it.each(['duplicate','nan','negative-volume','bad-ohlc','bad-session'])('blocks %s data',kind=>{
  const h=history();if(kind==='duplicate')h.minutes.push({...h.minutes[0]});if(kind==='nan')h.minutes[0].close=NaN;if(kind==='negative-volume')h.minutes[0].volume=-1;if(kind==='bad-ohlc')h.minutes[0].low=200;if(kind==='bad-session')h.sessions.push({...h.sessions[0]});
  expect(priceStructureBars(h,15,start+390*60000).bars).toEqual([]);
 });
 it('uses separate aggregation and ATR inputs for all five timeframes',()=>{
  const now=start+390*60000,s={symbol:'QQQ',publishedMs:now,chartHistory:history()} as LiveState;
  const cards=priceStructureCards(s,'QQQ',true,now);
  expect(cards.map(c=>c.interval)).toEqual([15,60,240,1440,10080]);expect(cards.map(c=>c.bars)).toEqual([26,7,2,0,0]);
  expect(cards[0].atr).not.toBeNull();expect(cards[1].atr).toBeNull();expect(cards[2].atr).toBeNull();
 });
 it.each(['inactive','wrong-symbol','stale','future'])('blocks %s frame evidence',kind=>{
  const now=start+390*60000,s={symbol:'QQQ',publishedMs:now,chartHistory:history()} as LiveState;
  if(kind==='wrong-symbol')s.symbol='SPY';if(kind==='stale')s.publishedMs-=15001;if(kind==='future')s.publishedMs++;
  expect(priceStructureCards(s,'QQQ',kind!=='inactive',now).every(c=>c.side==='SIN DATOS'&&c.bars===0)).toBe(true);
 });
});
