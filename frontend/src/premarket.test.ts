import {expect,it} from 'vitest';
import {premarketPanel,premarketWatch,type PremarketRead} from './premarket';
it('labels the cutoff and hides foreign, stale, future and previous-day context',()=>{
 const now=Date.parse('2026-09-24T14:00:00Z');
 const p:PremarketRead={symbol:'<QQQ>',asofMs:Date.parse('2026-09-24T13:15:00Z'),historical:false,status:'PREMARKET_ACTIVITY_AT_CUTOFF',price:101,premarketVolume:70,volume5m:70,premarketHigh:102,premarketLow:99,notional5mApprox:7070,lastBarEndMs:Date.parse('2026-09-24T13:15:00Z'),minutesInLast5m:3,activityReady:true,entryConfirmed:false};
 const rows=[{symbol:'<QQQ>',premarket:p}];
 const html=premarketPanel(rows,true,now);
 expect(html).toContain('&lt;QQQ&gt;');expect(html).toContain('Actividad observada');expect(html).toContain('Sin entrada confirmada');expect(html).toContain('no precio actual');
 expect(premarketPanel(rows,false,now)).not.toContain('Actividad observada');
 for(const changed of [{...p,symbol:'SPY'},{...p,historical:true},{...p,asofMs:now+1},{...p,asofMs:p.asofMs-86_400_000}]) {
  expect(premarketPanel([{symbol:'<QQQ>',premarket:changed}],true,now)).not.toContain('Actividad observada');
 }
});
it('expires the provisional list at the opening and rejects aged evidence',()=>{
 const now=Date.parse('2026-09-24T13:15:00Z');
 const p:PremarketRead={symbol:'<QQQ>',asofMs:now,historical:false,status:'PREMARKET_ACTIVITY_AT_CUTOFF',price:101,premarketVolume:70,volume5m:70,premarketHigh:102,premarketLow:99,notional5mApprox:7070,lastBarEndMs:now,minutesInLast5m:3,activityReady:true,entryConfirmed:false};
 expect(premarketWatch([p],true,now)).toContain('&lt;QQQ&gt;');
 for(const clock of [now-1,now+180_001,now+900_000]) expect(premarketWatch([p],true,clock)).not.toContain('&lt;QQQ&gt;');
 expect(premarketWatch([p],false,now)).not.toContain('&lt;QQQ&gt;');
});
