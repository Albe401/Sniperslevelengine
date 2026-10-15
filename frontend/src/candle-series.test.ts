import {it,expect} from 'vitest';
import {sessionCandles,currentSessionCandles,type ChartHistory} from './candle-series';
const open=Date.parse('2026-09-24T13:30:00Z');
const minute=(i:number,vwap:number|null=100)=>({timestampMs:open+i*60000,open:100+i,high:102+i,low:99+i,close:101+i,volume:10,vwap});
const history=(minutes:ReturnType<typeof minute>[]):ChartHistory=>({minutes,sessions:[{date:'2026-09-24',openMs:open,closeMs:open+390*60000}]});
it('scopes intraday calculations to the current session and remains causal across closes and corrections',()=>{
 const h=history([minute(0),minute(1),minute(2),minute(3)]);
 h.sessions.unshift({date:'2026-09-23',openMs:open-86400000,closeMs:open-86400000+390*60000});
 h.minutes.unshift({...minute(0),timestampMs:open-86400000});
 for(const at of [open+60000,open+119999,open+120000,open+240000])expect(currentSessionCandles(h,2,at)).toEqual(sessionCandles(h,[],2,at).filter(b=>b.sessionDate==='2026-09-24'));
 h.minutes[2].close=101.5;expect(currentSessionCandles(h,1,open+240000)[1].close).toBe(101.5);
 h.sessions.push({...h.sessions[1]});expect(currentSessionCandles(h,1,open+240000)).toEqual([]);
});
it('constructs closed 2m OHLCV and exact provider-weighted session VWAP',()=>{
 const rows=sessionCandles(history([minute(0,100),minute(1,102),minute(2,104),minute(3,106)]),[],2,open+240000);
 expect(rows).toHaveLength(2);expect(rows[0]).toMatchObject({open:100,high:103,low:99,close:102,volume:20,sessionVwap:101,complete:true});expect(rows[1].sessionVwap).toBe(103);
});
it('canonical minutes replace seconds without double counting; provisional VWAP stays unavailable',()=>{
 const rows=sessionCandles(history([minute(0)]),[{...minute(0),volume:999},{...minute(1),timestampMs:open+65000,volume:4}],2,open+70000);
 expect(rows[0].volume).toBe(14);expect(rows[0].provisional).toBe(true);expect(rows[0].sessionVwap).toBeNull();expect(rows[0].complete).toBe(false);
});
it('missing minute or provider VWAP invalidates session VWAP without fabricating a bar',()=>{
 for(const minutes of [[minute(0),minute(2),minute(3)],[minute(0,null),minute(1),minute(2),minute(3)]]){
 const rows=sessionCandles(history(minutes),[],2,open+240000);expect(rows.at(-1)!.sessionVwap).toBeNull();
 }
 expect(sessionCandles(history([minute(0),minute(3)]),[],2,open+240000).every(c=>!c.complete)).toBe(true);
});
it('respects schedule, rejects future data, and resets VWAP for a new session',()=>{
 const next=open+86400000;
 const h=history([minute(-1),minute(0,null),minute(390),{...minute(0,120),timestampMs:next},{...minute(1,122),timestampMs:next+60000}]);
 h.sessions.push({date:'2026-09-25',openMs:next,closeMs:next+210*60000});
 const rows=sessionCandles(h,[],2,next+120000);expect(rows).toHaveLength(2);expect(rows[1].sessionVwap).toBe(121);
 expect(sessionCandles(h,[],2,open).every(c=>c.timestampMs<=open)).toBe(true);
});
