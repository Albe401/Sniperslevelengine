import {it,expect} from 'vitest';
import {savedWeeklyRanges} from './saved-weekly-ranges';
import type {EtfReport} from './etf-dealers';
const at=Date.parse('2026-10-12T14:00:00Z');
const contract={expiration:'2026-10-16',strike:230,iv:.316,quoteAt:at-1000,realtime:true,bid:5,ask:5.2};
const report={symbol:'NVDA',days:183,at,spot:229.35,priceAt:at-1000,enrichment:{details:[{...contract,kind:'call'},{...contract,kind:'put'}]}} as unknown as EtfReport;
it('renders a saved range using the desktop calculation at observation time',()=>{
 const html=savedWeeklyRanges('NVDA',report,at+600000);
 expect(html).toContain('31.60%');expect(html).toContain('$229.35');expect(html).toContain('Límite inferior');expect(html).toContain('2026-10-16T16:00');expect(html).toContain('No es una lectura en vivo');
});
it('shows pending for missing, mismatched, future and stale quote inputs',()=>{
 for(const r of [undefined,{...report,symbol:'QQQ'},{...report,at:at+1},{...report,priceAt:at-120001}])expect(savedWeeklyRanges('NVDA',r,at)).toContain('Rango pendiente');
});
it('does not mutate the captured inventory and escapes the symbol',()=>{
 const before=JSON.stringify(report);savedWeeklyRanges('NVDA',report,at);expect(JSON.stringify(report)).toBe(before);
 expect(savedWeeklyRanges('<script>',undefined,at)).not.toContain('<script>');
});
