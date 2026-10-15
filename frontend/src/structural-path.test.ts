import {expect,it} from 'vitest';
import {structuralPathPanel,type StructuralPath} from './structural-path';
it('masks stale or unverified structural readiness and escapes labels',()=>{
 const row:StructuralPath={symbol:'<QQQ>',horizon:'DAY',magnet:100,targetWall:105,vacuumLow:101,vacuumHigh:102,trigger:{ready:true,reason:'<HOLD>',volume_ratio:1.2,reference_price:100.2,bar_end_ms:1000,vwap_supportive:true},candidate:{available:true,ready:true,side:'CALL',invalidation_level:99.9,risk_reward:4,flow_ready:true,participation_ready:true,barrier_blocked:false,blocks:[],entry_authorized:false}};
 const current=structuralPathPanel([row],true,1000,1000);expect(current).toContain('Condiciones alineadas');expect(current).toContain('&lt;HOLD&gt;');expect(current).toContain('&lt;QQQ&gt;');
 for(const [active,published,now] of [[false,1000,1000],[true,1000,4001],[true,181001,181001],[true,999,999]] as const){expect(structuralPathPanel([row],active,published,now)).not.toContain('Condiciones alineadas');}
 row.candidate.blocks=['UNVERIFIED_INPUT'];expect(structuralPathPanel([row],true,1000,1000)).not.toContain('Condiciones alineadas');
});
