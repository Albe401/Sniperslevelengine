import {expect,it} from 'vitest';
import {scannerOptionsPanel,type OptionsContext} from './scanner-options';
it('separates identities and only presents current complete structural fields',()=>{
 const now=Date.parse('2026-09-24T14:00:00Z');
 const row:OptionsContext={symbol:'QQQ',phase:'ready',detail:'<snapshot>',observedMs:now,referenceMs:now,contracts:20,entryConfirmed:false,horizons:[{mode:'DAY',observedMs:now,exposure:{gexComplete:true,netGex:12345},structure:{complete:true,putWall:{strike:490},callWall:{strike:510}}}]};
 const html=scannerOptionsPanel({QQQ:row},true,now);expect(html).toContain('&lt;snapshot&gt;');expect(html).toContain('490.00 / 510.00');
 for(const [rows,fresh,clock] of [[{SPY:row},true,now],[{QQQ:row},false,now],[{QQQ:row},true,now+900001],[{QQQ:{...row,observedMs:now+1}},true,now]] as const) {
  expect(scannerOptionsPanel(rows,fresh,clock)).not.toContain('490.00 / 510.00');
 }
 expect(scannerOptionsPanel({QQQ:{...row,horizons:[{...row.horizons[0],structure:{...row.horizons[0].structure,complete:false}}]}},true,now)).toContain('Estructura incompleta');
});
