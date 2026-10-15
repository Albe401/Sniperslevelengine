import { expect,it } from 'vitest';
import { confirmationPanel, type MagnetConfirmation } from './confirmations';
it('never presents a cached confirmation as current after IPC silence or disconnect',()=>{
  const c:MagnetConfirmation={status:'CONFIRMED_STRONG',reason:'<unsafe>',confirmationScore:90,alignment:'ALINEACIÓN FUERTE',primaryMagnet:500,offMagnet:500,distance:0,distancePct:0,entryConfirmed:false};
  expect(confirmationPanel({DAY:c},true,1000,1000)).toContain('Confirmación fuerte');
  expect(confirmationPanel({DAY:c},true,1000,1000)).toContain('&lt;unsafe&gt;');
  for(const [connected,now] of [[false,1000],[true,4001],[true,999]] as const){
    expect(confirmationPanel({DAY:c},connected,1000,now)).not.toContain('Confirmación fuerte');
  }
  expect(confirmationPanel({DAY:{...c,status:'WAITING'}},true,1000,1000)).not.toContain('90,00 / 100');
});
