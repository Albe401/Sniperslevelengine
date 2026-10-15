import {it,expect,vi} from 'vitest';
const api=vi.hoisted(()=>({invoke:vi.fn(),isTauri:()=>true}));
vi.mock('@tauri-apps/api/core',()=>api);
import {requestFourHour,currentFourHour,stopFourHour} from './route-4h-live';
it('refreshes after completion, exposes errors and clears them on a different asset',async()=>{
 stopFourHour();api.invoke.mockRejectedValueOnce('Estado no disponible');const refresh=vi.fn();
 requestFourHour('QQQ',true,100000,refresh);await vi.waitFor(()=>expect(refresh).toHaveBeenCalledOnce());
 expect(currentFourHour('QQQ',100000).summary).toContain('consulta falló: Estado no disponible');
 api.invoke.mockResolvedValueOnce(undefined);requestFourHour('SPY',true,100001,refresh);
 expect(currentFourHour('SPY',100001).summary).not.toContain('Estado no disponible');
 await vi.waitFor(()=>expect(refresh).toHaveBeenCalledTimes(2));stopFourHour();
});
it('limits pending requests and discards a response after leaving the view',async()=>{
 stopFourHour();let resolve!:(v:unknown)=>void;api.invoke.mockImplementationOnce(()=>new Promise(r=>{resolve=r;}));const refresh=vi.fn();const before=api.invoke.mock.calls.length;
 requestFourHour('QQQ',true,200000,refresh);requestFourHour('QQQ',true,210000,refresh);expect(api.invoke.mock.calls.length-before).toBe(1);
 stopFourHour();resolve(undefined);await new Promise(r=>setTimeout(r,0));expect(refresh).not.toHaveBeenCalled();
});
it('distinguishes rejected market freshness from a completed structure without a setup',async()=>{
 stopFourHour();const reading={version:'route-4h-v1',symbol:'QQQ',at:300000,structure:'range',state:'waiting',scenario:null,quality:{closedBlocks:20,incompleteBlocks:1}};
 api.invoke.mockResolvedValueOnce({available:false,mode:'review-only',executionEnabled:false,reading});
 const refresh=vi.fn();requestFourHour('QQQ',true,300000,refresh);await vi.waitFor(()=>expect(refresh).toHaveBeenCalledOnce());
 expect(currentFourHour('QQQ',300000).summary).toContain('precio o conexión no vigente');
 api.invoke.mockResolvedValueOnce({available:true,mode:'review-only',executionEnabled:false,reading:{...reading,at:310000}});
 requestFourHour('QQQ',true,310000,refresh);await vi.waitFor(()=>expect(refresh).toHaveBeenCalledTimes(2));
 expect(currentFourHour('QQQ',310000).summary).toContain('20 bloques cerrados');expect(currentFourHour('QQQ',310000).scenario).toBeUndefined();stopFourHour();
});
