import {expect,it} from 'vitest';
import {positionMapPanel} from './position-map';
it('never certifies cached geometry after IPC silence or disconnection',()=>{
  const r={ready:true,reason:'<reason>',price:100,putWall:90,callWall:110,pullTarget:105,mainMagnetRelation:'<magnet>',vacuumRelation:'DELANTE'};
  const html=positionMapPanel({'<CALL>':r},true,1000,1000);
  expect(html).toContain('Geometría válida');expect(html).toContain('&lt;reason&gt;');expect(html).toContain('&lt;magnet&gt;');
  for(const [active,now] of [[false,1000],[true,4001],[true,999]] as const)expect(positionMapPanel({CALL:r},active,1000,now)).not.toContain('Geometría válida');
  expect(positionMapPanel({CALL:{...r,ready:false}},true,1000,1000)).not.toContain('Geometría válida');
});
