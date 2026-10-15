import {afterEach,expect,it,vi} from 'vitest';
import {VisualCadence} from './visual-cadence';
afterEach(()=>vi.useRealTimers());
it('coalesces rapid ticks and renders the newest value at most once per second',()=>{
 vi.useFakeTimers();vi.setSystemTime(0);const c=new VisualCadence();let latest=0;const seen:number[]=[];const draw=()=>seen.push(latest);
 c.run(draw);for(let i=1;i<=9;i++){vi.advanceTimersByTime(100);latest=i;c.run(draw);}expect(seen).toEqual([0]);vi.advanceTimersByTime(100);expect(seen).toEqual([0,9]);
});
it('clears a queued reading immediately on disconnect and cancels updates on navigation',()=>{
 vi.useFakeTimers();vi.setSystemTime(0);const c=new VisualCadence();const draw=vi.fn();c.run(draw);vi.advanceTimersByTime(200);c.run(draw);const disconnected=vi.fn();c.run(disconnected,true);vi.advanceTimersByTime(1000);expect(draw).toHaveBeenCalledTimes(1);expect(disconnected).toHaveBeenCalledTimes(1);c.run(draw);c.run(draw);c.cancel();vi.advanceTimersByTime(1000);expect(draw).toHaveBeenCalledTimes(2);
});
