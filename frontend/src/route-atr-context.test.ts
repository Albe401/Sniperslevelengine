import {it,expect} from 'vitest';
import {routeAtrContext} from './route-atr-context';
import {intradayAtr,type AtrRead} from './intraday-atr';
import type {LiveState} from './live';
const now=Date.UTC(2026,9,8,15),a:AtrRead={value:2,percent:2,lastRangeRatio:1,endMs:now,reason:'test'};
it.each([[100,104,99],[100,96,101]])('measures both route directions without choosing a destination',(spot,target,stop)=>{
 const r=routeAtrContext(a,spot,target,stop,now);expect(r.targetAtr).toBe(2);expect(r.stopAtr).toBe(.5);expect(r.distance).toBe(4);
});
it.each([null,0,-1,NaN,Infinity])('rejects invalid ATR %s',value=>expect(routeAtrContext({...a,value},100,104,99,now).value).toBeNull());
it.each([now+1,now-300000,NaN])('rejects future or stale ATR timestamps %s',endMs=>expect(routeAtrContext({...a,endMs},100,104,99,now).value).toBeNull());
it('does not replace missing intraday ATR with daily ATR',()=>expect(routeAtrContext({...a,value:null,daily:{symbol:'QQQ',value:10,close:100,through:'2026-10-07',detail:''}},100,104,99,now).targetAtr).toBeNull());
it('a distant level remains measurable without an invented reachability cutoff',()=>expect(routeAtrContext(a,100,120,99,now).targetAtr).toBe(10));
it('handles missing levels and malformed prices',()=>{expect(routeAtrContext(a,100,null,null,now).value).toBe(2);expect(routeAtrContext(a,NaN,104,99,now).value).toBeNull();});
it('uses the real ATR engine and requires complete session minutes',()=>{
 const openMs=Date.UTC(2026,9,8,13,30),at=openMs+75*60000;
 const minutes=Array.from({length:75},(_,i)=>({timestampMs:openMs+i*60000,open:100,high:101,low:99,close:100,volume:100}));
 const s={symbol:'QQQ',publishedMs:at,stocks:{phase:'streaming'},metrics:{session:'2026-10-08',phase:'Sesión RTH'},chartHistory:{sessions:[{date:'2026-10-08',openMs,closeMs:openMs+390*60000}],minutes}} as unknown as LiveState;
 expect(routeAtrContext(intradayAtr(s,'QQQ',true,at),100,104,99,at).targetAtr).toBe(2);
 s.chartHistory!.minutes.splice(30,1);
 expect(routeAtrContext(intradayAtr(s,'QQQ',true,at),100,104,99,at).value).toBeNull();
});
