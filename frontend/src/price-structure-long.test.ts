import {describe,it,expect} from 'vitest';
import {longStructureBars} from './price-structure-long';
import {priceStructureBars,priceStructureSummary,PRICE_INTERVALS,type PriceStructureCard,type PriceStructureSide} from './price-structure';
import {mergeStructureMinutes,type PriceStructureHistory} from './price-structure-history';
const open=(date:string)=>Date.parse(date+'T09:30:00-04:00');
function long():PriceStructureHistory {
 const sessions=[];
 for(let t=Date.parse('2026-09-28T12:00Z');t<=Date.parse('2026-10-23T12:00Z');t+=86400000){const d=new Date(t);if([0,6].includes(d.getUTCDay()))continue;const date=d.toISOString().slice(0,10);sessions.push({date,openMs:open(date),closeMs:open(date)+390*60000});}
 const now=Date.parse('2026-10-24T12:00:00-04:00');
 return {symbol:'QQQ',asOfMs:now,throughMs:sessions.at(-1)!.closeMs,sessions,days:sessions.map((s,i)=>({date:s.date,open:100+i,high:102+i,low:99+i,close:101+i,volume:1000})),intraday:{sessions:[],minutes:[]}};
}
describe('long price-structure history',()=>{
 it('aggregates only whole closed exchange weeks, with actual first open and last close',()=>{
  const h=long(),weekly=longStructureBars(h,'QQQ',10080,h.asOfMs);
  expect(weekly.bars).toHaveLength(4);expect(weekly.bars[0]).toMatchObject({open:100,high:106,low:99,close:105,volume:5000,endMs:h.sessions[4].closeMs});
  expect(longStructureBars(h,'QQQ',1440,h.asOfMs).bars).toHaveLength(20);
 });
 it('does not treat a midweek snapshot as the closed weekly candle',()=>{
  const h=long();h.asOfMs=Date.parse('2026-10-21T12:00:00-04:00');h.days=h.days.filter(b=>b.date<'2026-10-21');h.throughMs=h.sessions.find(s=>s.date==='2026-10-20')!.closeMs;
  const r=longStructureBars(h,'QQQ',10080,h.asOfMs);
  expect(r.bars).toHaveLength(3);expect(r.bars.at(-1)!.sessionDate).toBe('2026-10-16');
 });
 it('uses a holiday-shortened exchange week and an early closing session without adding days',()=>{
  const h=long();h.sessions=h.sessions.filter(s=>s.date!=='2026-10-22');h.days=h.days.filter(s=>s.date!=='2026-10-22');h.sessions.at(-1)!.closeMs-=180*60000;h.throughMs=h.sessions.at(-1)!.closeMs;
  const last=longStructureBars(h,'QQQ',10080,h.asOfMs).bars.at(-1)!;
  expect(last.volume).toBe(4000);expect(last.endMs).toBe(h.throughMs);
 });
 it('resets both daily and weekly sequences at a missing official session, with no invented fallback',()=>{
  const h=long();h.days=h.days.filter(d=>d.date!=='2026-10-15');
  expect(longStructureBars(h,'QQQ',1440,h.asOfMs).bars).toHaveLength(6);
  expect(longStructureBars(h,'QQQ',10080,h.asOfMs).bars).toHaveLength(1);
  h.days=h.days.filter(d=>d.date!=='2026-10-23');
  expect(longStructureBars(h,'QQQ',1440,h.asOfMs).bars).toHaveLength(0);
  expect(longStructureBars(h,'QQQ',10080,h.asOfMs).bars).toHaveLength(0);
 });
 it.each(['foreign','duplicate','invalid','future','old','unfinished-refresh'])('rejects %s daily evidence',kind=>{
  const h=long();if(kind==='foreign')h.symbol='SPY';if(kind==='duplicate')h.days.push({...h.days[0]});if(kind==='invalid')h.days[0].high=1;if(kind==='future')h.asOfMs++;if(kind==='old')h.asOfMs-=86400000;if(kind==='unfinished-refresh')h.throughMs--;
  expect(longStructureBars(h,'QQQ',1440,Date.parse('2026-10-24T12:00:00-04:00')).bars).toHaveLength(0);
 });
 it('does not patch missing live minutes with cached historical minutes',()=>{
  const s={date:'2026-10-23',openMs:open('2026-10-23'),closeMs:open('2026-10-23')+390*60000};
  const older={sessions:[s],minutes:[{timestampMs:s.openMs,open:100,high:101,low:99,close:100,volume:1}]};
  expect(mergeStructureMinutes(older,{sessions:[s],minutes:[]}).minutes).toEqual([]);
 });
 it('keeps the 4h terminal 150-minute block and the shorter official early-close block',()=>{
  const start=open('2026-10-23'),h={sessions:[{date:'2026-10-23',openMs:start,closeMs:start+390*60000}],minutes:Array.from({length:390},(_,i)=>({timestampMs:start+i*60000,open:100,high:101,low:99,close:i===389?100.5:100,volume:1}))};
  const r=priceStructureBars(h,240,start+390*60000).bars;
  expect(r).toHaveLength(2);expect(r.map(b=>(b.endMs-b.timestampMs)/60000)).toEqual([240,150]);expect(r.at(-1)!.close).toBe(100.5);expect(r.reduce((s,b)=>s+b.volume,0)).toBe(390);
  h.sessions[0].closeMs=start+210*60000;
  expect(priceStructureBars(h,240,start+210*60000).bars.map(b=>(b.endMs-b.timestampMs)/60000)).toEqual([210]);
 });
});
describe('sixth card exact alignment',()=>{
 const cards=(sides:PriceStructureSide[])=>PRICE_INTERVALS.map((interval,i)=>({interval,side:sides[i]??'SIN DATOS'} as PriceStructureCard));
 it('requires all five directional states, rather than promoting a 3/5 majority',()=>{
  expect(priceStructureSummary(cards(['CALL','CALL','CALL','PUT','PUT']))).toMatchObject({side:'MIXTA',label:'DIVERGENCIA',call:3,put:2,missing:0});
  expect(priceStructureSummary(cards(['CALL','CALL','CALL','CALL','CALL']))).toMatchObject({side:'CALL',complete:5});
  expect(priceStructureSummary(cards(['PUT','PUT','PUT','PUT','PUT']))).toMatchObject({side:'PUT',complete:5});
 });
 it('cannot claim complete coverage with missing or duplicated horizons',()=>{
  const c=cards(['CALL','CALL','CALL','CALL','SIN DATOS']);expect(priceStructureSummary(c)).toMatchObject({side:'PARCIAL',complete:4,missing:1});
  c[4]={...c[0]};expect(priceStructureSummary(c)).toMatchObject({side:'PARCIAL',complete:0,missing:5});
 });
 it('conserves counts and coverage for every combination of the four input states',()=>{
  const states:PriceStructureSide[]=['CALL','PUT','MIXTA','SIN DATOS'];
  for(let n=0;n<1024;n++){
   const c=cards(PRICE_INTERVALS.map((_,i)=>states[(n>>(2*i))&3])),s=priceStructureSummary(c);
   expect(s.call+s.put+s.mixed+s.missing).toBe(5);expect(s.complete).toBe(5-s.missing);
   if(s.side==='CALL'||s.side==='PUT')expect(c.every(c=>c.side===s.side)).toBe(true);
   if(c.some(c=>c.side==='SIN DATOS'))expect(s.side).toBe('PARCIAL');
  }
 });
});
