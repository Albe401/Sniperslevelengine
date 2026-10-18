import {expect,it} from 'vitest';
import {capturedChartLevels} from './captured-chart-levels';
import type {LiveState} from './live';
const at=Date.parse('2026-10-18T20:00:00Z');
function frame(){return {symbol:'BAC',publishedMs:at,optionSnapshotStatus:{phase:'ready',horizons:[{mode:'NEXT_SESSION',observedMs:at-1000,structure:{complete:false,selected:{},provisional:{coveredContracts:2,totalContracts:4,callWall:{strike:54},putWall:{strike:50},magnet:52}}}]}} as unknown as LiveState;}
it('keeps three actual partial snapshot prices beside older candles without live scores',()=>{
 const f=frame(),before=JSON.stringify(f),levels=capturedChartLevels(f,'BAC',at,at);
 expect(levels.map(l=>l.price)).toEqual([54,50,52]);expect(levels.every(l=>l.importance===null&&l.name.includes('snapshot')&&l.name.includes('parcial'))).toBe(true);expect(JSON.stringify(f)).toBe(before);
});
it('never enables this path for live mode, another symbol, a mismatched replay clock or future evidence',()=>{
 expect(capturedChartLevels(frame(),'BAC',undefined,at)).toEqual([]);
 expect(capturedChartLevels(frame(),'QQQ',at,at)).toEqual([]);
 expect(capturedChartLevels(frame(),'BAC',at,at-1)).toEqual([]);
 const f=frame();f.optionSnapshotStatus.horizons[0].observedMs=at+1;expect(capturedChartLevels(f,'BAC',at,at)).toEqual([]);
});
