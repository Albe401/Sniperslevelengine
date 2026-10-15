import {describe,it,expect,vi} from 'vitest';
import {receiverHealth,transferConnection} from './data-connection';
describe('cambio manual de propietario de la conexión',()=>{
 it('espera salida de la vista anterior antes de conectar',async()=>{const order:string[]=[];let finish!:()=>void;const detach=()=>new Promise<void>(resolve=>{finish=()=>{order.push('salida');resolve();};});const attach=vi.fn(async()=>{order.push('entrada');});const transfer=transferConnection(detach,attach);expect(attach).not.toHaveBeenCalled();finish();await transfer;expect(order).toEqual(['salida','entrada']);});
 it('no inicia otra conexión si falla la desconexión',async()=>{const attach=vi.fn();await expect(transferConnection(async()=>{throw Error('falló');},attach)).rejects.toThrow('falló');expect(attach).not.toHaveBeenCalled();});
 it('distingue receptor vigente, congelado y desconectado sin usar ausencia de trades como fallo',()=>{expect(receiverHealth(true,10000,false,11000)).toBe('Receptor activo');expect(receiverHealth(true,10000,false,16000)).toContain('atrasada');expect(receiverHealth(true,10000,true,11000)).toContain('sin conexión');expect(receiverHealth(true,12000,false,11000)).toContain('atrasada');expect(receiverHealth(false,10000,false,16000)).toBe('');});
});
