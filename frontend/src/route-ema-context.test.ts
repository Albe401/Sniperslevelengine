import {it,expect} from 'vitest';
import {routeEmaContext} from './route-ema-context';
import type {LiveState} from './live';
const open=Date.parse('2026-10-09T09:30:00-04:00'),now=open+210*60000;
const fixture=()=>({publishedMs:now,stocks:{phase:'streaming'},chartHistory:{sessions:[{openMs:open,closeMs:open+390*60000}],minutes:Array.from({length:211},(_,i)=>({timestampMs:open+i*60000,open:100,close:i===210?900:100,high:i===210?901:101,low:99,volume:100}))}} as unknown as LiveState);
it('uses the bot EMA seed and ignores the unfinished bar',()=>{const r=routeEmaContext(fixture(),now);expect(r.available).toBe(true);expect([r.ema9,r.ema20,r.ema50,r.ema200]).toEqual([100,100,100,100]);});
it('rejects invalid publication clocks rather than treating NaN as current data',()=>{for(const value of [NaN,Infinity,now+1]){const s=fixture();s.publishedMs=value;expect(routeEmaContext(s,now).available).toBe(false);}});
it('rejects stale data, insufficient history, duplicates and recent gaps',()=>{for(const mode of ['stale','short','duplicate','gap']){const s=fixture();if(mode==='stale')s.publishedMs=now-3001;if(mode==='short')s.chartHistory!.minutes=s.chartHistory!.minutes.slice(-100);if(mode==='duplicate')s.chartHistory!.minutes.push({...s.chartHistory!.minutes[200]});if(mode==='gap')s.chartHistory!.minutes.splice(180,1);expect(routeEmaContext(s,now).available,mode).toBe(false);}});
