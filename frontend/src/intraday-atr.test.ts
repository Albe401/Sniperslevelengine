import {it,expect} from 'vitest';
import {intradayAtr,atrDistance,atrPanel} from './intraday-atr';
import {dayMap} from './day-map';
import type {LiveState} from './live';
const open=Date.UTC(2026,9,5,13,30),now=open+75*60000;
const fixture=()=>({symbol:'NVDA',publishedMs:now,stocks:{phase:'streaming'},metrics:{session:'2026-10-05',phase:'Sesión RTH'},chartHistory:{sessions:[{date:'2026-10-05',openMs:open,closeMs:open+390*60000}],minutes:Array.from({length:75},(_,i)=>({timestampMs:open+i*60000,open:100,high:101,low:99,close:100,volume:100}))}} as unknown as LiveState);
it('uses 14 true ranges with a real preceding close, not the forming candle',()=>{
 const s=fixture();s.chartHistory!.minutes.push({timestampMs:now,open:100,high:1000,low:1,close:100,volume:1});
 const a=intradayAtr(s,'NVDA',true,now);expect(a.value).toBe(2);expect(a.percent).toBe(2);expect(a.lastRangeRatio).toBe(1);expect(a.endMs).toBe(now);
});
it('includes a gap from preceding close and responds to corrected canonical candles',()=>{
 const s=fixture();for(const c of s.chartHistory!.minutes.slice(5)){c.open=110;c.high=111;c.low=109;c.close=110;}
 expect(intradayAtr(s,'NVDA',true,now).value).toBeCloseTo(37/14);
 s.chartHistory!.minutes[74].high=120;expect(intradayAtr(s,'NVDA',true,now).value!).toBeGreaterThan(37/14);
});
it.each(['warmup','missing','duplicate','ohlc','foreign','old-date','stale','closed','calendar'])('keeps %s data pending',kind=>{
 const s=fixture();let t=now;
 if(kind==='warmup'){t=now-300000;s.publishedMs=t;}if(kind==='missing')s.chartHistory!.minutes.splice(30,1);if(kind==='duplicate')s.chartHistory!.minutes.push({...s.chartHistory!.minutes[30]});if(kind==='ohlc')s.chartHistory!.minutes[30].low=-1;if(kind==='foreign')s.symbol='AAPL';if(kind==='old-date')s.metrics.session='2026-10-02';if(kind==='stale')s.publishedMs=now-3001;if(kind==='closed')s.metrics.phase='Cierre RTH';if(kind==='calendar')s.chartHistory!.sessions=[];
 expect(intradayAtr(s,'NVDA',true,t).value).toBeNull();
});
it('integrates ATR magnitude into the existing map without changing levels',()=>{
 const a=intradayAtr(fixture(),'NVDA',true,now),h={mode:'DAY',expirations:['2026-10-05'],structure:{complete:true,selected:{M1:103},callWall:{strike:104,score:80},putWall:{strike:96,score:80},routes:[]}} as unknown as LiveState['optionSnapshotStatus']['horizons'][number];
 expect(atrDistance(a,100,104)).toBe('2.00 ATR · 5 min');expect(atrDistance({...a,value:null},100,104)).toBe('');
 const html=dayMap(h,true,[],false,100,'',undefined,true,'',undefined,a);expect(html).toContain('2.00 ATR · 5 min');expect(html).toContain('104.00');expect(atrPanel(a)).toContain('Mide magnitud');
});
it('benchmarks 3000 ATR and map updates against a larger minute history',()=>{
 const s=fixture();s.chartHistory!.minutes.unshift(...Array.from({length:5000},(_,i)=>({...s.chartHistory!.minutes[0],timestampMs:open-(i+1)*60000})));
 const h={mode:'DAY',expirations:['2026-10-05'],structure:{complete:true,selected:{M1:103},callWall:{strike:104,score:80},putWall:{strike:96,score:80},routes:[]}} as unknown as LiveState['optionSnapshotStatus']['horizons'][number];
 s.chartHistory!.sessions.unshift({date:'2026-10-02',openMs:Date.UTC(2026,9,2,13,30),closeMs:Date.UTC(2026,9,2,20)});s.dailyAtr={symbol:'NVDA',through:'2026-10-02',value:4,close:100,detail:'15 sesiones'};
 const start=performance.now();for(let i=0;i<3000;i++){const a=intradayAtr(s,'NVDA',true,now);dayMap(h,true,[],false,100,'',undefined,true,'',undefined,a);}
 const elapsed=performance.now()-start;console.info(`ATR + mapa, 5075 minutos: 3000 actualizaciones ${elapsed.toFixed(1)} ms; ${(elapsed/3000).toFixed(3)} ms/actualización`);expect(elapsed).toBeLessThan(15000);
});
it('shows daily ATR independently of intraday warmup and rejects stale or foreign daily data',()=>{
 const s=fixture();s.chartHistory!.sessions.unshift({date:'2026-10-02',openMs:Date.UTC(2026,9,2,13,30),closeMs:Date.UTC(2026,9,2,20)});
 s.dailyAtr={symbol:'NVDA',through:'2026-10-02',value:4,close:100,detail:'15 sesiones completas'};
 expect(intradayAtr(s,'NVDA',true,now).daily?.value).toBe(4);
 s.metrics.phase='Esperando primera vela RTH completa';expect(atrPanel(intradayAtr(s,'NVDA',true,now))).toContain('$4.00</strong>');
 s.dailyAtr.symbol='AAPL';expect(intradayAtr(s,'NVDA',true,now).daily).toBeUndefined();
 s.dailyAtr.symbol='NVDA';s.dailyAtr.through='2026-10-01';expect(intradayAtr(s,'NVDA',true,now).daily).toBeUndefined();
});


