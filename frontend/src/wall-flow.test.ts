import {expect,it} from 'vitest';
import {wallFlowPanel} from './wall-flow';
it('shows partial observed totals without certifying stale zones or unknown premium',()=>{
  const c={levelsRequested:3,levelsCovered:2,coveragePct:66.7,contracts:0,premium:null,trades:0};
  const reports={'<DAY>':{available:true,windows:[{minutes:1,callZone:c,putZone:c,callPull:c,putPull:c}]}};
  const html=wallFlowPanel(reports,true,1000,1000,1000);
  expect(html).toContain('&lt;DAY&gt;');expect(html).toContain('2 / 3 · 66.70 %');expect(html).toContain('<td>0.00</td><td>—</td>');
  for(const [active,published,event,now] of [[false,1000,1000,1000],[true,1000,1000,4001],[true,17000,1000,17000],[true,1000,1001,1000]] as const){expect(wallFlowPanel(reports,active,published,event,now)).not.toContain('<td>0.00</td>');}
  reports['<DAY>'].available=false;
  expect(wallFlowPanel(reports,true,1000,1000,1000)).not.toContain('66.70');
});
