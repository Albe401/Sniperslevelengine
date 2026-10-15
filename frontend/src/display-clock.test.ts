import {it,expect} from 'vitest';
import {selectedPrice} from './selected-price';
import {buildIntradayControl} from './intraday-control';
import type {LiveState} from './live';
it('keeps live price and model coverage stable across bounded provider clock jitter',()=>{
 const now=Date.parse('2026-09-28T15:30:00Z');
 const s={symbol:'QQQ',publishedMs:now,stocks:{phase:'streaming'},options:{phase:'streaming',lastEventMs:now},ticks:{QQQ:{symbol:'QQQ',price:735.7,tradeMs:now}},metrics:{session:'2026-09-28'},chartHistory:{minutes:[{timestampMs:now-120000,open:736,high:736,low:735,close:735,volume:10}],sessions:[]},flow:{intraday_windows:[1,5].map(minutes=>({minutes,event_count:5,premium_complete:true,bullish_pct:90,bearish_pct:10,buckets:{'CALL BUY':{premium:900},'PUT BUY':{premium:100}}}))}} as unknown as LiveState;
 const coverage=buildIntradayControl(s,'QQQ',true,now).coverage;
 expect(coverage).toBeGreaterThan(0);
 for(const lead of [650,-20,550,0,1000,-100]){
  s.ticks.QQQ.tradeMs=now+lead;s.options.lastEventMs=now+lead;
  expect(selectedPrice('QQQ',s,true,now)).toMatchObject({live:true,value:735.7});
  expect(buildIntradayControl(s,'QQQ',true,now).coverage).toBe(coverage);
 }
 s.ticks.QQQ.tradeMs=now+1001;s.options.lastEventMs=now+1001;
 expect(selectedPrice('QQQ',s,true,now).live).toBe(false);
 expect(buildIntradayControl(s,'QQQ',true,now).coverage).toBe(0);
 s.ticks.QQQ.tradeMs=now-15001;s.options.lastEventMs=now-15001;
 expect(selectedPrice('QQQ',s,true,now).live).toBe(false);
 expect(buildIntradayControl(s,'QQQ',true,now).coverage).toBe(0);
});
