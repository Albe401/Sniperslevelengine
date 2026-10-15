import {expect,it} from 'vitest';
import {gammaChartData} from './gamma-chart-data';
import type {LiveState} from './live';
const open=Date.parse('2026-10-01T13:30:00Z'),now=open+120*60000;
const minute=(i:number,t=open)=>({timestampMs:t+i*60000,open:100+i,high:102+i,low:99+i,close:101+i,volume:10,vwap:100+i});
function fixture(){return {symbol:'QQQ',bars:{},chartHistory:{sessions:[{date:'2026-10-01',openMs:open,closeMs:open+390*60000}],minutes:Array.from({length:120},(_,i)=>minute(i))}} as unknown as LiveState;}
it.each([1,2,5,15,60,1440])('aggregates actual bars at %sm without fabricating data',n=>{const d=gammaChartData(fixture(),'QQQ',true,n,now);expect(d.bars.length).toBe(n===1440?1:120/n);expect(d.bars.reduce((sum,b)=>sum+b.volume,0)).toBe(1200);expect(d.bars.every(b=>b.timestampMs<=now)).toBe(true);expect(d.vwap.at(-1)!.value).toBeCloseTo(159.5);expect(d.currentOverlay).toBe(true);});
it('session VWAP never bridges missing or invalid provider data',()=>{for(const kind of ['gap','missing-vwap']){const s=fixture();if(kind==='gap')s.chartHistory!.minutes.splice(15,1);else s.chartHistory!.minutes[15].vwap=null;const d=gammaChartData(s,'QQQ',true,60,now);expect(d.vwap.at(-1)!.value).toBeNull();}});
it('history is kept but current exposure is hidden for a past session',()=>{const d=gammaChartData(fixture(),'QQQ',true,2,now+86400000);expect(d.bars).toHaveLength(60);expect(d.historical).toBe(true);expect(d.currentOverlay).toBe(false);});
it('intraday and daily retain separate historical sessions',()=>{const s=fixture(),prior=open-86400000;s.chartHistory!.sessions.unshift({date:'2026-09-30',openMs:prior,closeMs:prior+390*60000});s.chartHistory!.minutes.unshift(...Array.from({length:60},(_,i)=>minute(i,prior)));expect(gammaChartData(s,'QQQ',true,2,now).bars).toHaveLength(90);const daily=gammaChartData(s,'QQQ',true,1440,now);expect(daily.bars).toHaveLength(2);expect(daily.vwap.map(v=>v.value)).toEqual([129.5,159.5]);});
it.each(['inactive','foreign','missing'])('removes previous asset bars for %s',kind=>{const d=gammaChartData(kind==='missing'?undefined:fixture(),kind==='foreign'?'AAPL':'QQQ',kind!=='inactive',2,now);expect(d.bars).toEqual([]);expect(d.currentOverlay).toBe(false);});
it('future bars are rejected and input data is not mutated',()=>{const s=fixture();s.chartHistory!.minutes.push(minute(999));const before=JSON.stringify(s);const d=gammaChartData(s,'QQQ',true,2,now);expect(d.bars).toHaveLength(60);expect(JSON.stringify(s)).toBe(before);expect(()=>gammaChartData(s,'QQQ',true,7,now)).toThrow();});
it('hour buckets start at RTH open, and daily dates remain the correct ET date',()=>{const s=fixture();const hour=gammaChartData(s,'QQQ',true,60,now);expect(hour.bars.map(b=>b.timestampMs)).toEqual([open,open+3600000]);const daily=gammaChartData(s,'QQQ',true,1440,now);expect(new Date(daily.bars[0].timestampMs).toLocaleDateString('en-CA',{timeZone:'America/New_York'})).toBe('2026-10-01');});

it('technical bars reject unfinished or incomplete hourly buckets',()=>{const s=fixture();expect(gammaChartData(s,'QQQ',true,60,now).closedBars).toHaveLength(2);s.chartHistory!.minutes.splice(15,1);expect(gammaChartData(s,'QQQ',true,60,now).closedBars).toHaveLength(1);expect(gammaChartData(fixture(),'QQQ',true,60,open+90*60000).closedBars).toHaveLength(1);});
it('daily technical bars require a complete closed provider session',()=>{const s=fixture();expect(gammaChartData(s,'QQQ',true,1440,now).closedBars).toEqual([]);s.chartHistory!.minutes=Array.from({length:390},(_,i)=>minute(i));expect(gammaChartData(s,'QQQ',true,1440,open+391*60000).closedBars).toHaveLength(1);});

it('4h preserves sessions and closes the shorter RTH final bar only when all minutes are present',()=>{const s=fixture(),prior=open-86400000;s.chartHistory!.sessions.unshift({date:'2026-09-30',openMs:prior,closeMs:prior+390*60000});s.chartHistory!.minutes=[...Array.from({length:390},(_,i)=>minute(i,prior)),...Array.from({length:390},(_,i)=>minute(i))];const d=gammaChartData(s,'QQQ',true,240,open+391*60000);expect(d.bars.map(b=>b.timestampMs)).toEqual([prior,prior+240*60000,open,open+240*60000]);expect(d.closedBars).toHaveLength(4);expect(d.bars.map(b=>b.volume)).toEqual([2400,1500,2400,1500]);expect(gammaChartData(s,'QQQ',true,240,open+300*60000).closedBars).toHaveLength(3);s.chartHistory!.minutes.splice(250,1);expect(gammaChartData(s,'QQQ',true,240,open+391*60000).closedBars).toHaveLength(3);});

it('handles thirty full sessions without slow chart preparation',()=>{
 const s=fixture(),sessions=[],minutes=[];
 for(let d=0;d<45&&sessions.length<30;d++){
  const at=Date.parse('2026-08-03T13:30:00Z')+d*86400000;
  if([0,6].includes(new Date(at).getUTCDay()))continue;
  const date=new Date(at).toISOString().slice(0,10);
  sessions.push({date,openMs:at,closeMs:at+390*60000});
  minutes.push(...Array.from({length:390},(_,i)=>minute(i,at)));
 }
 s.chartHistory={sessions,minutes};
 const end=sessions.at(-1)!.closeMs+60000;
 const durations=[];
 for(const interval of [1,2,5,15,60,240,1440]){
  const start=performance.now(),result=gammaChartData(s,'QQQ',true,interval,end);
  durations.push(performance.now()-start);
  expect(result.bars.length).toBe(interval===1440?30:interval===240?60:interval===60?210:30*390/interval);
 }
 console.info('CHART_30_SESSION_MS',JSON.stringify({minutes:minutes.length,intervals:[1,2,5,15,60,240,1440],durations:durations.map(x=>Math.round(x))}));
 expect(Math.max(...durations)).toBeLessThan(500);
});
