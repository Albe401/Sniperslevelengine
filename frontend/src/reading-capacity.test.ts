import {expect,it} from 'vitest';
import {buildIntradayControl} from './intraday-control';
import type {LiveState} from './live';

it('retains the current reading when five sessions of history are loaded',()=>{
 const now=Date.parse('2026-10-02T15:00:20Z');
 const dates=['2026-09-28','2026-09-29','2026-09-30','2026-10-01','2026-10-02'];
 const sessions=dates.map(date=>({date,openMs:Date.parse(date+'T13:30:00Z'),closeMs:Date.parse(date+'T20:00:00Z')}));
 const minutes=sessions.flatMap((s,index)=>Array.from({length:index===4?90:390},(_,i)=>({timestampMs:s.openMs+i*60000,open:738-i*.05,high:738.1-i*.05,low:737.8-i*.05,close:737.9-i*.05,volume:1000,vwap:738-i*.05})));
 const s={symbol:'QQQ',publishedMs:now,startedMs:now-600000,stocks:{phase:'streaming'},options:{phase:'denied'},ticks:{QQQ:{symbol:'QQQ',price:732.8,tradeMs:now}},metrics:{session:'2026-10-02',phase:'Sesión RTH',asofMs:now,vwap:737},flow:{intraday_windows:[]},chartHistory:{sessions,minutes}} as unknown as LiveState;
 const current={...s,chartHistory:{sessions:sessions.slice(-1),minutes:minutes.filter(m=>m.timestampMs>=sessions[4].openMs)}};
 const expected=buildIntradayControl(current,'QQQ',true,now);
 const start=performance.now();
 for(let i=0;i<5;i++)expect(buildIntradayControl(s,'QQQ',true,now)).toEqual(expected);
 console.log('READING_CAPACITY',JSON.stringify({bars:minutes.length,reads:5,elapsedMs:performance.now()-start}));
});
