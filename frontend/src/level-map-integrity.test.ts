import {it,expect} from 'vitest';
import {levelQuality} from './level-quality';
import {levelOpenInterest} from './level-open-interest';
import type {EtfReport} from './etf-dealers';
it('keeps scores normalized on a partial global universe explicitly provisional even at complete local strikes',()=>{
 const r=levelQuality([{strike:710,total:2,covered:2,oiCoverage:100,missingOi:0,missingGamma:0,missingVolume:0}],710,710,true);
 expect(r.provisional).toBe(true);
});
it('does not ignore an invalid option kind while claiming a verified per-strike composition',()=>{
 const r={expirations:['2026-10-26'],enrichment:{details:[{ticker:'c',expiration:'2026-10-26',strike:710,kind:'call',oi:100},{ticker:'p',expiration:'2026-10-26',strike:710,kind:'put',oi:200},{ticker:'bad',expiration:'2026-10-26',strike:710,kind:'other',oi:10000}]}} as unknown as EtfReport;
 expect(levelOpenInterest(r,710)).toBeNull();
});

it.each(['strike','expiry','side','root','date'])('rejects %s metadata mismatches in OCC option series',kind=>{
 const call={ticker:'O:QQQ261026C00710000',expiration:'2026-10-26',strike:710,kind:'call',oi:100};
 if(kind==='strike')call.ticker='O:QQQ261026C00711000';if(kind==='expiry')call.ticker='O:QQQ261027C00710000';if(kind==='side')call.ticker='O:QQQ261026P00710000';if(kind==='root')call.ticker='O:SPY261026C00710000';if(kind==='date')call.ticker='O:QQQ260230C00710000';
 const r={symbol:'QQQ',expirations:['2026-10-26'],enrichment:{details:[call,{ticker:'O:QQQ261026P00710000',expiration:'2026-10-26',strike:710,kind:'put',oi:200}]}} as unknown as EtfReport;
 expect(levelOpenInterest(r,710)).toBeNull();
});
