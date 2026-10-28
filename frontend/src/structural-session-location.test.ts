import {describe,expect,it} from 'vitest';
import type {LiveState} from './live';
import {fiveMinuteStructure} from './price-structure';
import {readSessionLocation,sessionLocationDetails} from './structural-session-location';
const open=Date.parse('2026-10-27T09:30:00-04:00'),now=open+1200000;
function state(){const previous=Date.parse('2026-10-26T09:30:00-04:00');return{symbol:'QQQ',publishedMs:now,metrics:{session:'2026-10-27',phase:'Sesión RTH',vwap:null,asofMs:null},stocks:{phase:'streaming'},chartHistory:{sessions:[{date:'2026-10-26',openMs:previous,closeMs:previous+23400000},{date:'2026-10-27',openMs:open,closeMs:open+23400000}],minutes:[...Array.from({length:390},(_,i)=>({timestampMs:previous+i*60000,open:97,high:98,low:96,close:97,volume:10,vwap:97})),...Array.from({length:20},(_,i)=>({timestampMs:open+i*60000,open:100,high:101+i*.01,low:99+i*.01,close:100.2,volume:10,vwap:100+i*.01}))]}}as unknown as LiveState;}
describe('closed5m session location with existing source helpers',()=>{
 it('reuses opening range and providerVWAP at the exact close with prior-session coverage',()=>{
  const s=state(),before=JSON.stringify(s),five=fiveMinuteStructure(s,'QQQ',true,now);five.atr=2;
  const r=readSessionLocation(s,'QQQ',true,now,five);
  expect(r.opening).toMatchObject({high:101.14,low:99,closedAtMs:open+900000});expect(r.previous).toMatchObject({date:'2026-10-26',high:98,low:96,close:97});
  expect(r.vwap?.value).toBeCloseTo(100.095,10);expect(r.vwap?.difference).toBeCloseTo(.105,10);expect(r.vwap?.distanceAtr).toBeCloseTo(.0525,10);expect(r.vwap?.atMs).toBe(now);
  expect(r.opening?.position).toBeCloseTo(1.2/2.14,10);expect(r.nearestAbove?.name).toBe('Rango apertura · alto');expect(r.entryAuthorized).toBe(false);expect(JSON.stringify(s)).toBe(before);
 });
 it('keepsVWAPmissing without eligible provider values and rejects a later metrics cut',()=>{
  const s=state();for(const b of s.chartHistory!.minutes)b.vwap=null;s.metrics.vwap=102;s.metrics.asofMs=now+60000;
  const five=fiveMinuteStructure(s,'QQQ',true,now);expect(readSessionLocation(s,'QQQ',true,now,five).vwap).toBeNull();
  s.metrics.asofMs=now;expect(readSessionLocation(s,'QQQ',true,now,five).vwap?.source).toBe('existing-session-metrics');
 });
 it('does not create opening or cumulativeVWAP across a missing minute',()=>{
  const s=state();s.chartHistory!.minutes=s.chartHistory!.minutes.filter(b=>b.timestampMs!==open+480000);const r=readSessionLocation(s,'QQQ',true,now,fiveMinuteStructure(s,'QQQ',true,now));expect(r.opening).toBeNull();expect(r.vwap).toBeNull();
 });
 it.each(['inactive','foreign','future','different-publication']as const)('rejects %s association',kind=>{
  const s=state(),five=fiveMinuteStructure(s,'QQQ',true,now);if(kind==='foreign')five.sourceSymbol='SPY';if(kind==='future')five.endMs=now+1;if(kind==='different-publication')five.sourcePublishedMs=now-1;
  expect(readSessionLocation(s,'QQQ',kind!=='inactive',now,five).availability).toBe('pending');
 });
 it('renders missing evidence as pending without authorizing an entry',()=>{
  const s=state(),r=readSessionLocation(s,'QQQ',false,now,fiveMinuteStructure(s,'QQQ',false,now));expect(sessionLocationDetails(r)).toContain('data-session-location="pending"');expect(r.probability).toBeNull();
 });
});
