import {expect,it} from 'vitest';
import {flowCoveragePanel} from './flow-coverage';
it('keeps observed zero distinct from unknown and masks stale coverage',()=>{
  const row={ticker:'<ticker>',kind:'call',strike:100,expiration:'2026-09-24',minutes:1,covered:true,contracts:0,premium:0,trades:0,reason:'<reason>'};
  const ready=flowCoveragePanel([row],true,1000,1000,1000);
  expect(ready).toContain('Ventana observada<');expect(ready).toContain('<td>0.00</td>');expect(ready).toContain('&lt;ticker&gt;');
  for(const [active,published,last,now] of [[false,1000,1000,1000],[true,1000,1000,4001],[true,17000,1000,17000],[true,1000,1001,1000]] as const){expect(flowCoveragePanel([row],active,published,last,now)).not.toContain('<td>0.00</td>');}
  expect(flowCoveragePanel([{...row,covered:false,contracts:null,premium:null}],true,1000,1000,1000)).not.toContain('<td>0.00</td>');
});
