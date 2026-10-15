import {expect,it} from 'vitest';
import {wallPullPanel} from './wall-pull';
it('masks stale price and DAY flow without requiring live options for structural horizons',()=>{
 const read={state:'LISTO PARA CALL',ready:true,reason:'<razón>',supportReady:true,pullReady:true,staticReady:true,flowAligned:true,liveRequired:true,liveReady:true,liveWindowUsed:1,entryConfirmed:false};
 const panel=(stock:number,option:number,now:number)=>wallPullPanel({'<DAY>':read},true,now,stock,option,now);
 expect(panel(1000,1000,1000)).toContain('LISTO PARA CALL');expect(panel(1000,1000,1000)).toContain('&lt;razón&gt;');
 expect(panel(1000,17000,17000)).not.toContain('LISTO PARA CALL');expect(panel(17000,1000,17000)).not.toContain('LISTO PARA CALL');
 expect(panel(1001,1000,1000)).not.toContain('LISTO PARA CALL');
 read.liveRequired=false;read.state='ESTRUCTURA CALL FAVORABLE';
 expect(panel(17000,1000,17000)).toContain('ESTRUCTURA CALL FAVORABLE');
 expect(wallPullPanel({DAY:read},false,1000,1000,1000,1000)).not.toContain('ESTRUCTURA CALL FAVORABLE');
});
