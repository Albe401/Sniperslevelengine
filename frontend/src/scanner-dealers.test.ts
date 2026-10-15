import {it,expect} from 'vitest';
import {scannerDealers,scannerDealerScore} from './scanner-dealers';
import {demoPayload} from '../../integrations/neuro-trader-engine/runtime/frontend/src/lib/demo';
import type {NeuroContext} from './neuro-context';
const c:NeuroContext={symbol:'QQQ',at:100000,available:true,reason:'',netGamma:1,charmFlow:1,gammaMagnet:500,gammaFlip:499,contracts:100,used:80,modelPayload:demoPayload('QQQ',200)};
it('exposes all dealer and magnet readings for the same current asset',()=>{
 const html=scannerDealers(c,'QQQ',true,100000);
 for(const label of ['Control de dealers','Prueba del Call Wall','Prueba del Put Wall','Presión de pin','Deriva por charm','FUERZA','CANDIDATOS','Max pain','Zona de aceleración'])expect(html).toContain(label);
 expect(scannerDealerScore(c,'QQQ',true,100000)).toBeGreaterThanOrEqual(0);
});
it('never ranks absent, mismatched, expired or disconnected profiles as neutral evidence',()=>{
 for(const [context,symbol,active,now] of [[undefined,'QQQ',true,100000],[c,'SPY',true,100000],[c,'QQQ',false,100000],[c,'QQQ',true,145001],[c,'QQQ',true,99999]] as const)expect(scannerDealerScore(context,symbol,active,now)).toBeNull();
 expect(scannerDealers(c,'QQQ',false,100000)).toContain('prioridad —');
});
