import {expect,it,vi} from 'vitest';
const {invoke}=vi.hoisted(()=>({invoke:vi.fn().mockResolvedValue(undefined)}));
vi.mock('@tauri-apps/api/core',()=>({invoke,isTauri:()=>true}));
import {setEtfScannerVisible} from './etf-dealers';
it('activates only on entering and serializes exit and re-entry without chain queries',async()=>{
 setEtfScannerVisible(false);expect(invoke).not.toHaveBeenCalled();
 setEtfScannerVisible(true);setEtfScannerVisible(true);setEtfScannerVisible(false);setEtfScannerVisible(true);
 await vi.waitFor(()=>expect(invoke).toHaveBeenCalledTimes(3));
 expect(invoke.mock.calls).toEqual([
  ['set_etf_scanner_enabled',{enabled:true}],
  ['set_etf_scanner_enabled',{enabled:false}],
  ['set_etf_scanner_enabled',{enabled:true}],
 ]);
 setEtfScannerVisible(false);await vi.waitFor(()=>expect(invoke).toHaveBeenCalledTimes(4));
});
