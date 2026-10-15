import {afterEach,expect,it,vi} from 'vitest';
const native=vi.hoisted(()=>({invoke:vi.fn(),isTauri:vi.fn(()=>true)}));
vi.mock('@tauri-apps/api/core',()=>native);
afterEach(()=>{vi.resetModules();vi.clearAllMocks();vi.unstubAllGlobals();});
it('polls native status without overlapping requests or advancing any formula',async()=>{
 vi.stubGlobal('window',{});
 native.invoke.mockResolvedValue({status:'OBSERVING',audit:{summary:{certifiedVolumeContractWindows:6,certifiedPremiumContractWindows:6,bothComparablePriceCompatibleContractWindows:0}}});
 const reader=await import('./option-research-live');
 reader.pollOptionResearch(100000);reader.pollOptionResearch(100001);
 await new Promise(resolve=>setTimeout(resolve,0));
 expect(native.invoke).toHaveBeenCalledTimes(1);
 expect(native.invoke).toHaveBeenCalledWith('get_option_research_status');
 expect(reader.optionResearchNote()).toContain('comparables 0');
});
it('keeps a failed backend status visible',async()=>{
 vi.stubGlobal('window',{});native.invoke.mockRejectedValue(new Error('offline'));
 const reader=await import('./option-research-live');reader.pollOptionResearch(100000);
 await new Promise(resolve=>setTimeout(resolve,0));
 expect(reader.optionResearchNote()).toContain('no disponible');
});
