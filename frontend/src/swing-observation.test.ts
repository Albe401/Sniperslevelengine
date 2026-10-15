import {describe,it,expect} from 'vitest';
import {swingObservation} from './swing-observation';
import type {LiveState} from './live';
const now=10000;
const frame=()=>({symbol:'QQQ',swingReport:{mode:'observation-only',state:'observing',context:{version:'structural-swing-v1',symbol:'QQQ',at:now,side:'CALL',blockers:[]},intraday:{state:'supported'},reason:'Contexto <vigente>',remainingDollars:3}} as unknown as LiveState);
describe('passive swing display',()=>{
 it('shows fresh scoped observation and escapes text',()=>{const v=swingObservation(frame(),'QQQ',true,now);expect(v.label).toContain('CALL');expect(v.html).toContain('&lt;vigente&gt;');expect(v.html).toContain('sin ejecución automática');});
 it('rejects inactive, foreign, future and stale reports',()=>{expect(swingObservation(frame(),'QQQ',false,now).label).toContain('PENDIENTE');expect(swingObservation(frame(),'SPY',true,now).label).toContain('PENDIENTE');expect(swingObservation(frame(),'QQQ',true,now-1).label).toContain('PENDIENTE');expect(swingObservation(frame(),'QQQ',true,now+5001).label).toContain('PENDIENTE');});
});
