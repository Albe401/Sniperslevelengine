import {expect,it,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {bindAnalystTargets,loadTargets} from './analyst-targets';
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>true,invoke:vi.fn()}));
it('completes a coverage scan without waiting for another market tick and expires cached targets',async()=>{
 vi.useFakeTimers();vi.setSystemTime(new Date('2026-10-02T18:00:00Z'));
 const row={symbol:'NVDA',status:'Disponible',fetchedMs:Date.now(),low:100,average:120,high:140};
 vi.mocked(invoke).mockResolvedValueOnce({configured:true,running:true,total:169,status:'Consultando',rows:{}}).mockResolvedValue({configured:true,running:false,total:169,status:'Comprobado',rows:{NVDA:row}});
 const refresh=vi.fn();bindAnalystTargets(refresh);bindAnalystTargets(refresh);
 await vi.advanceTimersByTimeAsync(0);expect(invoke).toHaveBeenCalledTimes(1);
 await vi.advanceTimersByTimeAsync(5000);expect(invoke).toHaveBeenCalledTimes(2);
 expect(loadTargets('NVDA')?.average).toBe(120);expect(loadTargets('AAPL')).toBeUndefined();expect(loadTargets('NVDA')?.average).toBe(120);expect(invoke).toHaveBeenCalledTimes(2);
 vi.setSystemTime(new Date('2026-10-03T03:59:59Z'));expect(loadTargets('NVDA')?.average).toBe(120);
 vi.setSystemTime(new Date('2026-10-03T04:00:00Z'));expect(loadTargets('NVDA')).toBeUndefined();
 vi.setSystemTime(new Date(row.fetchedMs+86400000));expect(loadTargets('NVDA')).toBeUndefined();
 vi.clearAllTimers();vi.useRealTimers();
});
