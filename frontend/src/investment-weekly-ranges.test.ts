import {it,expect,vi} from 'vitest';
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>true,invoke:vi.fn()}));
import {invoke} from '@tauri-apps/api/core';
import {bindWeeklyRanges} from './investment-weekly-ranges';
import {weeklyRange,easternTimestamp,weeklyRangesPanel,scannedWeeklyEntry,weeklyScanSymbols} from './investment-weekly-ranges';
it('calculates calendar-day expected move with IV percentage',()=>{const r=weeklyRange(229.35,31.6,7)!;expect(r.move).toBeCloseTo(10.036,2);expect(r.lower+r.upper).toBeCloseTo(458.7);expect(weeklyRange(229.35,31.6,1)!.move*Math.sqrt(7)).toBeCloseTo(r.move);});
it('rejects missing or reversed weekly inputs',()=>{for(const values of [[0,31.6,7],[229,NaN,7],[229,0,7],[229,31.6,0],[229,31.6,8]])expect(weeklyRange(...values as [number,number,number])).toBeNull();});
it('uses elapsed calendar time across ET DST and rejects ambiguous local timestamps',()=>{expect((easternTimestamp('2026-11-02T16:00')-easternTimestamp('2026-10-30T16:00'))/3600000).toBe(73);expect(easternTimestamp('2026-11-01T01:30')).toBeNaN();expect(easternTimestamp('2026-03-08T02:30')).toBeNaN();});
it('shows missing data instead of example prices as results',()=>{const html=weeklyRangesPanel();expect(html).toContain('Pendiente de datos');expect(html).not.toContain('$229.35');expect(html).toContain('Escanear rangos semanales');});
it('uses price and ATM IV from the same snapshot and rejects stale or missing inputs',()=>{
 const now=Date.parse('2026-10-12T14:00:00Z');
 const detail={ticker:'O:NVDA',expiration:'2026-10-16',strike:230,iv:.316,quoteAt:now-1000,realtime:true,bid:5,ask:5.2};
 const report={symbol:'NVDA',days:7,at:now,spot:229.35,priceAt:now-1000,enrichment:{details:[{...detail,kind:'call'},{...detail,kind:'put'}]}} as unknown as import('./etf-dealers').EtfReport;
 const entry=scannedWeeklyEntry(report,'NVDA',now);expect(entry.price).toBe(229.35);expect(entry.iv).toBeCloseTo(31.6);expect(entry.expiry).toBe('2026-10-16T16:00');
 expect(()=>scannedWeeklyEntry({...report,priceAt:now-120001},'NVDA',now)).toThrow();
 expect(()=>scannedWeeklyEntry(report,'QQQ',now)).toThrow();
 const bad=structuredClone(report);bad.enrichment!.details[0].iv=null;expect(()=>scannedWeeklyEntry(bad,'NVDA',now)).toThrow();
 const spread=structuredClone(report);spread.enrichment!.details[0].ask=10;expect(()=>scannedWeeklyEntry(spread,'NVDA',now)).toThrow();
});
it('allows a coherent last-session reference on Sunday, without weakening live freshness',()=>{
 const now=Date.parse('2026-10-11T18:00:00Z'),close=Date.parse('2026-10-09T20:00:00Z'),open=Date.parse('2026-10-09T13:30:00Z');
 const d={ticker:'O:NVDA',expiration:'2026-10-16',strike:230,iv:.316,quoteAt:close-60000,realtime:true,bid:5,ask:5.2};
 const r={symbol:'NVDA',days:7,at:now,spot:229.35,priceAt:close,historicalSession:{date:'2026-10-09',openMs:open,closeMs:close},enrichment:{details:[{...d,kind:'call'},{...d,kind:'put'}]}} as unknown as import('./investment-weekly-ranges').WeeklySnapshot;
 const entry=scannedWeeklyEntry(r,'NVDA',now);expect(entry.source).toContain('Referencia histórica');expect(entry.days).toBe(7);expect(entry.reference).toBe('2026-10-09T16:00');
 expect(()=>scannedWeeklyEntry({...r,historicalSession:null},'NVDA',now)).toThrow();
 const bad=structuredClone(r);bad.enrichment!.details[0].quoteAt=close-86400000;expect(()=>scannedWeeklyEntry(bad,'NVDA',now)).toThrow();
 const early=structuredClone(r);early.enrichment!.details[0].quoteAt=close-16*60000;expect(()=>scannedWeeklyEntry(early,'NVDA',now)).toThrow();
});
it('offers a deduplicated major-company universe and filters by sector',()=>{
 const all=weeklyScanSymbols('sectors','all','');expect(all.length).toBe(36);expect(new Set(all).size).toBe(all.length);for(const s of ['NVDA','AAPL','MSFT','AMZN','META','JPM'])expect(all).toContain(s);
 const tech=weeklyScanSymbols('sectors','TECNOLOGIA','');expect(tech).toContain('NVDA');expect(tech).not.toContain('JPM');
 expect(weeklyScanSymbols('symbols','all','nvda, MSFT nvda')).toEqual(['NVDA','MSFT']);
});
it('uses the supplied early-close calendar instead of silently assuming 16:00 ET',()=>{const now=Date.parse('2026-11-23T15:00:00Z');const close=Date.parse('2026-11-27T18:00:00Z');const d={ticker:'O:NVDA',expiration:'2026-11-27',strike:230,iv:.316,quoteAt:now-1000,realtime:true,bid:5,ask:5.2};const r={symbol:'NVDA',days:7,at:now,spot:229.35,priceAt:now-1000,expirationCloses:{'2026-11-27':close},enrichment:{details:[{...d,kind:'call'},{...d,kind:'put'}]}} as unknown as import('./investment-weekly-ranges').WeeklySnapshot;const entry=scannedWeeklyEntry(r,'NVDA',now);expect(entry.expiry).toBe('2026-11-27T13:00');expect(entry.days).toBeCloseTo((close-now+1000)/86400000);expect(()=>scannedWeeklyEntry({...r,expirationCloses:{}},'NVDA',now)).toThrow();const bad=structuredClone(r);bad.enrichment!.details[0].iv=null;expect(()=>scannedWeeklyEntry(bad,'NVDA',now)).toThrow('CALL: IV ausente');});
it('rejects approximation ranges with impossible negative lower prices or overflow',()=>{expect(weeklyRange(100,1000,7)).toBeNull();expect(weeklyRange(Number.MAX_VALUE,1000,7)).toBeNull();expect(weeklyRange(100,31.6,7)!.lower).toBeGreaterThan(0);});

it('retries a failed visible scan and removes the old error without overlapping requests',async()=>{
 vi.useFakeTimers();
 const status={textContent:''},host={innerHTML:''},button={disabled:false,textContent:''};
 const controls={mode:{value:'symbols'},sector:{value:'all'},symbols:{value:'NBIS'}};
 const form={querySelector:(q:string)=>q==='button'?button:controls[q.match(/name="(.*?)"/)![1] as keyof typeof controls],onsubmit:null as unknown as (e:{preventDefault:()=>void})=>Promise<void>};
 const panel={querySelector:(q:string)=>q.includes('error')?status:q.includes('cards')?host:button};
 const root={querySelector:(q:string)=>q.includes('scan')?form:q.includes('error')?status:host};
 vi.stubGlobal('document',{visibilityState:'visible',querySelector:()=>panel});
 const now=Date.parse('2026-10-12T18:00:00Z');vi.setSystemTime(now);
 const d={expiration:'2026-10-16',strike:207.5,iv:.74,quoteAt:now,realtime:true,bid:5,ask:5.2};
 vi.mocked(invoke).mockRejectedValueOnce(Error('Cadena vacía')).mockResolvedValue({symbol:'NBIS',days:7,at:now,spot:207.5,priceAt:now,enrichment:{details:[{...d,kind:'call'},{...d,kind:'put'}]}});
 try{
  bindWeeklyRanges(root as unknown as HTMLElement);await form.onsubmit({preventDefault(){}});
  expect(host.innerHTML).toContain('Cadena vacía');
  await vi.advanceTimersByTimeAsync(60000);
  expect(invoke).toHaveBeenCalledTimes(2);expect(host.innerHTML).not.toContain('Cadena vacía');expect(host.innerHTML).toContain('$207.50');
  vi.stubGlobal('document',{visibilityState:'hidden',querySelector:()=>panel});await vi.advanceTimersByTimeAsync(60000);expect(invoke).toHaveBeenCalledTimes(2);
 }finally{vi.clearAllTimers();vi.useRealTimers();vi.unstubAllGlobals();vi.mocked(invoke).mockReset();}
});

it('identifies the failed input without conflating missing chain and stale price',()=>{
 const now=Date.parse('2026-10-12T18:00:00Z');const base={symbol:'NBIS',days:7,at:now,spot:207.5,priceAt:now,enrichment:{details:[]}} as unknown as import('./investment-weekly-ranges').WeeklySnapshot;
 expect(()=>scannedWeeklyEntry(base,'NBIS',now)).toThrow('Cadena de opciones de Massive vacía');
 expect(()=>scannedWeeklyEntry({...base,priceAt:now-120001},'NBIS',now)).toThrow('Precio de Massive caducado');
 expect(()=>scannedWeeklyEntry({...base,priceAt:NaN},'NBIS',now)).toThrow('Fecha del precio');
 expect(()=>scannedWeeklyEntry({...base,spot:0},'NBIS',now)).toThrow('Precio de Massive ausente');
 expect(()=>scannedWeeklyEntry({...base,at:now-300001},'NBIS',now)).toThrow('Consulta de Massive caducada');
 expect(()=>scannedWeeklyEntry(base,'QQQ',now)).toThrow('incompatible con el activo');
});

it('includes every stock and ETF in the watchlist scan, including NBIS and benchmarks',()=>{
 const all=weeklyScanSymbols('watchlist','all','');expect(all.length).toBe(187);expect(new Set(all).size).toBe(all.length);for(const symbol of ['NBIS','SPCX','NVDA','QQQ','SPY','IWM'])expect(all).toContain(symbol);expect(all).toContain('SPX');expect(weeklyRangesPanel()).toContain('Todo el watchlist');
});
