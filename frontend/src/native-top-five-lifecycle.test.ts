import {it,expect,vi,afterEach} from 'vitest';
const api=vi.hoisted(()=>({invoke:vi.fn()}));
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>true,invoke:api.invoke}));
import {refreshNativeOptions,setNativeHorizon,nativeReports} from './native-top-five';
const rows=[{symbol:'AAPL',price:100,volume:100,changePct:1}];
afterEach(()=>{vi.unstubAllGlobals();api.invoke.mockReset();setNativeHorizon(7);});
it('uses only the native command, queues horizon changes and rejects the old response',async()=>{
 vi.stubGlobal('document',{querySelector:()=>({})});
 let complete!:(v:unknown)=>void;
 api.invoke.mockImplementationOnce(()=>new Promise(resolve=>{complete=resolve;})).mockResolvedValue({reports:{},errors:{}});
 const paint=vi.fn(),pending=refreshNativeOptions(rows,paint,true);
 expect(api.invoke.mock.calls[0]).toEqual(['get_manual_scanner_profiles',{symbols:['AAPL'],maxDte:7}]);
 setNativeHorizon(0);await refreshNativeOptions(rows,paint,true);
 complete({reports:{AAPL:{symbol:'AAPL',days:7}},errors:{}});await pending;await Promise.resolve();
 expect(api.invoke).toHaveBeenCalledTimes(2);expect(api.invoke.mock.calls[1][1].maxDte).toBe(0);expect(nativeReports.AAPL).toBeUndefined();
 expect(api.invoke.mock.calls.some(call=>String(call[0]).includes('neuro'))).toBe(false);
});
it('does not query when the scanner is hidden',async()=>{
 vi.stubGlobal('document',{querySelector:()=>null});await refreshNativeOptions(rows,vi.fn(),true);expect(api.invoke).not.toHaveBeenCalled();
});
