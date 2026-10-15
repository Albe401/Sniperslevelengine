import {describe,it,expect,beforeEach} from 'vitest';
import {blockPressurePanel,selectBlockPeriod,defaultBlockPeriod,type BlockReport} from './block-pressure';
import type {LiveState} from './live';
const now=Date.parse('2026-10-01T14:00:00Z');
function fixture(){const p={count:3,total:450000,buy:200000,sell:100000,unknown:150000,buy_pct:66.67,sell_pct:33.33,coverage_pct:66.67,dominant:'BUY',complete:true,finalized:true,through_ms:now,top_levels:[]};return {symbol:'QQQ',publishedMs:now,offExchange:{blockQuoteStatus:'<error>',analytics:{blocks:{symbol:'QQQ',date:'2026-10-01',minimum_notional:100000,excluded_time:0,premarket:p,regular:{...p,buy:100000,sell:200000}} as BlockReport}}} as unknown as LiveState;}
describe('Block pressure quality',()=>{
 beforeEach(()=>selectBlockPeriod('QQQ','premarket'));
 it('defaults to the New York session at 09:30 and resets to premarket on the next day',()=>{
  for(const [date,period] of [['2026-10-01T13:29:59Z','premarket'],['2026-10-01T13:30:00Z','regular'],['2026-10-02T00:00:00Z','regular'],['2026-10-02T08:00:00Z','premarket'],['2026-11-02T14:29:59Z','premarket'],['2026-11-02T14:30:00Z','regular']] as const)expect(defaultBlockPeriod(Date.parse(date))).toBe(period);
 });
 it('shows classified shares separately from coverage without asserting an uncertain lead',()=>{const html=blockPressurePanel(fixture(),'QQQ',true,now);expect(html).toContain('66.7%');expect(html).toContain('33.3%');expect(html).toContain('Sin clasificar');expect(html).not.toContain('Domina presión compradora');expect(html).toContain('&lt;error&gt;');});
 it('never presents another ticker, stale publication or prior date as current blocks',()=>{for(const s of [ {...fixture(),symbol:'SPY'}, {...fixture(),publishedMs:now-10000},fixture()]){if(s.symbol==='QQQ'&&s.publishedMs===now)s.offExchange.analytics.blocks!.date='2026-09-30';const html=blockPressurePanel(s,'QQQ',true,now);expect(html).not.toContain('66.7%');expect(html).toContain('Esperando datos propios');}});
 it('keeps premarket and regular choices separate',()=>{selectBlockPeriod('QQQ','regular');const html=blockPressurePanel(fixture(),'QQQ',true,now);expect(html).toContain('Sesión regular ET');expect(html).toContain('33.3%');selectBlockPeriod('QQQ','premarket');});
 it('shows no direction when everything is unclassified',()=>{const s=fixture();const p=s.offExchange.analytics.blocks!.premarket;p.buy=0;p.sell=0;p.unknown=p.total;const html=blockPressurePanel(s,'QQQ',true,now);expect(html).toContain('Dirección sin clasificar');expect(html).not.toContain('50.0%');});
});
