import {describe,it,expect} from 'vitest';
import {legacyPositions,type LegacyPosition} from './legacy-positions';
describe('legacy continuation',()=>{it('distinguishes archived prices, live observations, paused management and closed positions',()=>{
 const position:LegacyPosition={legacyId:12,symbol:'<MSFT>',transferredMs:1000,trade:{side:'CALL',entry:100,current:101,stop:98,target1:102,target2:105,pnlPoints:1,highest:101,lowest:100,mfe:1,mae:0,lastMs:1000,closedMs:null,exitReason:null}};
 expect(legacyPositions([position],true,true,1001)).toContain('Último valor guardado');
 position.trade.lastMs=1001;
 expect(legacyPositions([position],true,true,1002)).toContain('Abierta · precio vigente');
 expect(legacyPositions([position],false,true,1002)).toContain('esperando precio vigente');
 expect(legacyPositions([position],true,true,17000)).toContain('esperando precio vigente');
 position.trade.closedMs=1002;position.trade.exitReason='<STOP>';
 const html=legacyPositions([position],false,false,2000);
 expect(html).toContain('Cerrada');expect(html).toContain('&lt;MSFT&gt;');expect(html).toContain('&lt;STOP&gt;');
});});
