import {expect,it} from 'vitest';
import {optionLiquidityPanel} from './option-liquidity';
it('masks disconnected and expired liquidity evidence and escapes labels',()=>{
 const d={status:'<LIQUIDEZ ACEPTABLE>',ready:true,contractsTracked:10,quoted:10,recent:7,depth:5,quoteCoveragePct:100,recentCoveragePct:70,depthCoveragePct:50,averageSpreadPct:10,entryConfirmed:false};
 expect(optionLiquidityPanel(d,'<QQQ>',true,1000,1000)).toContain('&lt;LIQUIDEZ ACEPTABLE&gt;');
 expect(optionLiquidityPanel(d,'<QQQ>',true,1000,1000)).toContain('&lt;QQQ&gt;');
 for(const [on,now] of [[false,1000],[true,4001],[true,999]] as const) {
  const html=optionLiquidityPanel(d,'QQQ',on,1000,now);
  expect(html).not.toContain('&lt;LIQUIDEZ ACEPTABLE&gt;');expect(html).not.toContain('70.00 %');
 }
});
