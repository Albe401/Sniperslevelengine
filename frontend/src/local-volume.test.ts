import {it,expect} from 'vitest';
import {localVolumePanel} from './local-volume';
it('local pace never survives identity, freshness or connection failure',()=>{
 const r={symbol:'QQQ',state:'FUERTE',usable:true,rvol_local:2,contracts_1m:200,baseline_contracts_per_minute:100,call_contracts_1m:100,put_contracts_1m:100,last_trade_age_seconds:14,reason:'<local>'};
 expect(localVolumePanel(r,'QQQ',true,1000,1000)).toContain('FUERTE');
 expect(localVolumePanel(r,'QQQ',true,1000,1000)).toContain('&lt;local&gt;');
 expect(localVolumePanel(r,'QQQ',true,1000,3001)).toContain('Operaciones caducadas');
 expect(localVolumePanel(r,'QQQ',true,1000,5001)).toContain('Publicación caducada');
 for(const [symbol,active,now] of [['SPY',true,1000],['QQQ',false,1000],['QQQ',true,3001],['QQQ',true,999]] as const)expect(localVolumePanel(r,symbol,active,1000,now)).not.toContain('>FUERTE<');
});
