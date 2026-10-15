import { expect, it } from 'vitest';
import { dailyRangePanel, rangeFresh, type RangeSnapshot } from './daily-range';
const now=Date.parse('2026-09-24T10:30:00-04:00');
const snapshot:RangeSnapshot={phase:'ready',observedMs:now,dailyRangeSource:'<source>',dailyRange:{
  ready:true,method:'ATM_IV_1D',spot:100,nearestExpiration:'2026-09-24',atmStrike:100,
  atmIv:0.2,expectedMoveDollars:1.26,expectedMovePct:1.26,snapshotLower:98.74,snapshotUpper:101.26,
  atmStraddle:6,straddleDailyEquivalent:6,ivStraddleDivergencePct:376,tradingDaysToExpiration:1,
  callTicker:'<call>',putTicker:'put',dataQuality:'HIGH',notes:['<note>']}};
it('hides bands without a current successful snapshot and escapes external labels',()=>{
  const html=dailyRangePanel(snapshot,true,now);
  expect(html).toContain('98.74');
  expect(html).toContain('&lt;source&gt;');
  expect(html).toContain('&lt;call&gt;');
  expect(html).toContain('&lt;note&gt;');
  for(const phase of ['loading','failed']) expect(dailyRangePanel({...snapshot,phase},true,now)).not.toContain('98.74');
  expect(dailyRangePanel(snapshot,false,now)).not.toContain('98.74');
  expect(dailyRangePanel(snapshot,true,now+900_001)).not.toContain('98.74');
  expect(dailyRangePanel({...snapshot,dailyRange:null,dailyRangeError:'<error>'},true,now)).toContain('&lt;error&gt;');
});
it('expires at the age limit or ET date change and rejects future observations',()=>{
  expect(rangeFresh(snapshot,now+900_000)).toBe(true);
  expect(rangeFresh(snapshot,now+900_001)).toBe(false);
  expect(rangeFresh(snapshot,now-1)).toBe(false);
  const midnight=Date.parse('2026-09-25T00:00:00-04:00');
  expect(rangeFresh({...snapshot,observedMs:midnight-1},midnight)).toBe(false);
});
