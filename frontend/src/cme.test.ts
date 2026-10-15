import {it,expect} from 'vitest';
import {cmePanel,type CmeStatus} from './cme';
it('keeps unlicensed CME inactive and revokes stale conversion',()=>{
  expect(cmePanel(undefined,false,undefined)).toContain('SIN LICENCIA');
  const now=Date.parse('2026-09-24T15:00:00Z');
  const state:CmeStatus={phase:'ready',detail:'Conversión aproximada',environment:'UAT',contract:'NQZ6',nqPrice:20000,nqTimestampMs:now,conversions:[{source:'NQ_QQQ_OBSERVED_RATIO',approximate:true,nqLevel:20100,qqqLevel:502.5,ratio:0.025,oldestPriceMs:now}],reconnects:0};
  expect(cmePanel(state,true,now,now)).toContain('≈');
  expect(cmePanel(state,true,now,now)).toContain('UAT');
  for(const html of [cmePanel(state,false,now,now),cmePanel(state,true,now-3001,now),cmePanel(state,true,now+15001,now+15001)])expect(html).not.toContain('≈');
  state.conversions[0].source='OTHER';expect(cmePanel(state,true,now,now)).not.toContain('≈');
});
