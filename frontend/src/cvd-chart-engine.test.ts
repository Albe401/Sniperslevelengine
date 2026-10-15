import {describe,it,expect} from 'vitest';
import {CvdChartEngine} from './cvd-chart-engine';
import type {LiveState} from './live';
const t=Date.UTC(2026,9,8,14);
const frame=(at:number,delta:number,gaps=0)=>({symbol:'QQQ',stocks:{phase:'streaming'},metrics:{phase:'Sesión RTH'},cvd:{symbol:'QQQ',date:'2026-10-08',available:true,at,regular:{buy:Math.max(0,delta),sell:Math.max(0,-delta),unknown:0,delta,lastMs:at,sinceMs:t,gaps}}} as unknown as LiveState);
const bars=Array.from({length:6},(_,i)=>({timestampMs:t+i*60000}));
it('restores the real observed curve when opening the chart after several minutes',()=>{
 const e=new CvdChartEngine(),s=frame(t+180000,15);
 s.cvd!.regular.history=[{at:t+59999,value:10},{at:t+119999,value:-20},{at:t+179999,value:5},{at:t+180000,value:15}];
 e.observe(s,'QQQ',true,t+180000);
 expect(e.series(bars,1).slice(0,4).map(p=>p.value)).toEqual([10,-20,5,15]);
 expect(e.series(bars.filter((_,i)=>i%5===0),5)[0].value).toBe(15);
});
it('does not trust fabricated, unordered or inconsistent historical totals',()=>{
 for(const history of [[{at:t-1,value:10}],[{at:t,value:999}],[{at:t+1,value:5},{at:t,value:10}]]){
  const e=new CvdChartEngine(),s=frame(t+120000,10);s.cvd!.regular.history=history;
  e.observe(s,'QQQ',true,t+120000);
  expect(e.series(bars,1)[0].value).toBeUndefined();
 }
});
it('matches the backend bounded 1-second provider clock lead without hiding valid live CVD',()=>{
 const e=new CvdChartEngine(),s=frame(t,10);s.cvd!.regular.lastMs=t+600;
 e.observe(s,'QQQ',true,t);expect(e.series(bars,1)[0].value).toBe(10);
 const rejected=new CvdChartEngine();s.cvd!.regular.lastMs=t+1001;
 rejected.observe(s,'QQQ',true,t);expect(rejected.series(bars,1).every(p=>p.value==null)).toBe(true);
});
describe('Observed CVD chart',()=>{
 it('keeps historical candles empty and updates the observed minute without duplicates',()=>{const e=new CvdChartEngine();e.observe(frame(t+120000,30),'QQQ',true,t+120000);e.observe(frame(t+121000,50),'QQQ',true,t+121000);const p=e.series(bars,1);expect(p[0]).not.toHaveProperty('value');expect(p[2].value).toBe(50);expect(p.filter(x=>x.value!=null)).toHaveLength(1);});
 it('rejects stale and foreign evidence',()=>{const e=new CvdChartEngine();e.observe(frame(t,10),'SPY',true,t);expect(e.series(bars,1).every(x=>x.value==null)).toBe(true);e.observe(frame(t,10),'QQQ',true,t+4000);expect(e.series(bars,1).every(x=>x.value==null)).toBe(true);});
 it('leaves a break across reconnects',()=>{const e=new CvdChartEngine();e.observe(frame(t,10),'QQQ',true,t);e.observe(frame(t+60000,20,1),'QQQ',true,t+60000);expect(e.series(bars,1)[1]).not.toHaveProperty('value');});
 it('resets the series when the active symbol changes',()=>{const e=new CvdChartEngine();e.observe(frame(t,10),'QQQ',true,t);e.observe(undefined,'SPY',false,t);expect(e.series(bars,1).every(x=>x.value==null)).toBe(true);});
 it('uses the last accumulated reading per interval without summing accumulated totals',()=>{const e=new CvdChartEngine();e.observe(frame(t,10),'QQQ',true,t);e.observe(frame(t+1000,25),'QQQ',true,t+1000);expect(e.series(bars.filter((_,i)=>i%5===0),5)[0].value).toBe(25);expect(e.series(bars,1)[1]).not.toHaveProperty('value');});
 it('rejects inconsistent directional totals',()=>{const e=new CvdChartEngine(),s=frame(t,10);s.cvd!.regular.delta=99;e.observe(s,'QQQ',true,t);expect(e.series(bars,1).every(x=>x.value==null)).toBe(true);});
 it('starts a fresh curve for the next session',()=>{const e=new CvdChartEngine();e.observe(frame(t,10),'QQQ',true,t);const s=frame(t+86400000,20);s.cvd!.date='2026-10-09';e.observe(s,'QQQ',true,t+86400000);expect(e.series(bars,1).every(x=>x.value==null)).toBe(true);});
});

it.each(['wrong-date','after-hours','invalid-gaps','missing-since'])('rejects %s curve evidence',kind=>{const e=new CvdChartEngine(),s=frame(t,10);if(kind==='wrong-date')s.cvd!.date='2026-10-07';if(kind==='after-hours')s.metrics.phase='Cerrado';if(kind==='invalid-gaps')s.cvd!.regular.gaps=-1;if(kind==='missing-since')s.cvd!.regular.sinceMs=null;e.observe(s,'QQQ',true,t);expect(e.series(bars,1).every(p=>p.value==null)).toBe(true);});
it('breaks the curve when the accumulation origin changes without a gap count change',()=>{const e=new CvdChartEngine();e.observe(frame(t+59000,100),'QQQ',true,t+59000);const s=frame(t+61000,5);s.cvd!.regular.sinceMs=t+60000;e.observe(s,'QQQ',true,t+61000);expect(e.series(bars,1)[1]).not.toHaveProperty('value');});
