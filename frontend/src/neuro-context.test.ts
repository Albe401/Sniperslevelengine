import {describe,it,expect} from 'vitest';
import {neuroText,type NeuroContext} from './neuro-context';
const c:NeuroContext={symbol:'QQQ',at:100000,available:true,reason:'',netGamma:-10,charmFlow:-1,gammaMagnet:500,gammaFlip:499,contracts:100,used:80};
describe('Neuro context',()=>{
 it('describes a model and exposes divergence without confirming entry',()=>{const t=neuroText(c,'QQQ',true,100000,'CALL');expect(t).toContain('Divergencia');expect(t).toContain('posible amplificación');expect(t).toContain('no flujo observado');});
 it('rejects other symbols, disconnected, future and stale contexts',()=>{for(const [s,a,n] of [['SPY',true,100000],['QQQ',false,100000],['QQQ',true,99999],['QQQ',true,145001]] as const)expect(neuroText(c,s,a,n)).toContain('no vigente');});
 it('does not imply long-term horizon confirmation',()=>expect(neuroText(c,'QQQ',true,100000)).toContain('siete días'));
});
