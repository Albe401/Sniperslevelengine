import {it,expect,vi,afterEach} from 'vitest';
const api=vi.hoisted(()=>({invoke:vi.fn()}));
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>true,invoke:api.invoke,Channel:class {onmessage?:unknown}}));
import {bindManualScanner,manualSearch} from './manual-scanner';
afterEach(()=>{vi.unstubAllGlobals();api.invoke.mockReset();manualSearch.busy=false;manualSearch.error='';manualSearch.report=undefined;});
it('invokes the real command once while busy and releases the button after errors',async()=>{
 const button:{onclick?:()=>Promise<void>}={};const root={innerHTML:''};
 vi.stubGlobal('document',{querySelector:(s:string)=>s==='[data-manual-scan]'?button:root});
 let fail!:(e:unknown)=>void;
 api.invoke.mockImplementation(()=>new Promise((_,reject)=>{fail=reject;}));
 bindManualScanner();const pending=button.onclick!();
 await button.onclick!();expect(api.invoke).toHaveBeenCalledTimes(1);
 expect(api.invoke.mock.calls[0][0]).toBe('search_top_five_now');expect(manualSearch.busy).toBe(true);
 fail('Sin permiso');await pending;
 expect(manualSearch.busy).toBe(false);expect(root.innerHTML).toContain('Sin permiso');
 expect(root.innerHTML).toContain('>Buscar ahora');
});
