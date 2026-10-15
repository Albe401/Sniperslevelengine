import {expect,it} from 'vitest';
import {buyPremiumPercentages,flowFour} from './flow-four';
import type {LiveState} from './live';
it('computes shares of buy premium, not contract counts or coverage',()=>{
 expect(buyPremiumPercentages(600,400,true)).toEqual({call:60,put:40});
 expect(buyPremiumPercentages(0,400,true)).toEqual({call:0,put:100});
 expect(buyPremiumPercentages(600,0,true)).toEqual({call:100,put:0});
});
it('never invents percentages for missing, invalid, empty or partial premiums',()=>{
 for(const [call,put,complete] of [[0,0,true],[600,400,false],[undefined,400,true],[-1,2,true],[NaN,400,true],[Infinity,1,true],[Number.MAX_VALUE,Number.MAX_VALUE,true]] as const)expect(buyPremiumPercentages(call,put,complete)).toBeNull();
});
it('shows percentages in the existing labels and hides them after disconnect',()=>{
 const now=Date.now(),s={symbol:'QQQ',publishedMs:now,options:{phase:'streaming',lastEventMs:now},flow:{intraday_windows:[{minutes:5,event_count:2,premium_complete:true,buckets:{'CALL BUY':{premium:600},'PUT BUY':{premium:400},UNKNOWN:{premium:500}}}]}} as unknown as LiveState;
 const html=flowFour(s,'QQQ',true,now,undefined,false);
 expect(html).toContain('CALL BUY · 60.00%');expect(html).toContain('PUT BUY · 40.00%');expect(html.match(/flow-four-cell /g)).toHaveLength(2);expect(html).toContain('sin clasificar $500');
 for(const h of [flowFour(s,'SPY',true,now),flowFour(s,'QQQ',false,now),flowFour(s,'QQQ',true,now+3001)])expect(h).not.toContain('60.00%');
 s.flow.intraday_windows![0].premium_complete=false;expect(flowFour(s,'QQQ',true,now)).not.toContain('60.00%');
});
