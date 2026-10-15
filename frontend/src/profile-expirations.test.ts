import {expect,it} from 'vitest';
import {profileExpirations} from './profile-expirations';
import {scopeContractReport} from './contract-profile';
import type {EtfReport} from './etf-dealers';
const now=Date.parse('2026-10-05T10:00:00-04:00');
const dates=['2026-10-12','2026-10-07','2026-10-05','2026-10-06','2026-10-09','2026-10-02'];
const report={enrichment:{details:dates.flatMap((expiration,i)=>['call','put'].map(kind=>({expiration,kind,strike:500,ticker:expiration+kind,oi:kind==='call'?20000:15000,volume:i+1,gex:10,dex:20})))}} as unknown as EtfReport;
it('selects today, current calendar week, or four unique future expiry dates',()=>{
 expect(profileExpirations(report,'DAY',now)).toEqual(['2026-10-05']);
 expect(profileExpirations(report,'WEEK',now)).toEqual(['2026-10-05','2026-10-06','2026-10-07','2026-10-09']);
 expect(profileExpirations(report,'NEXT4',now)).toEqual(['2026-10-05','2026-10-06','2026-10-07','2026-10-09']);
 const thursday=Date.parse('2026-10-08T10:00:00-04:00');
 expect(profileExpirations(report,'DAY',thursday)).toEqual([]);
 expect(profileExpirations(report,'WEEK',thursday)).toEqual(['2026-10-09']);
});
it('aggregates only selected contracts for OI, volume and GEX',()=>{
 const scoped=scopeContractReport(report,profileExpirations(report,'NEXT4',now));
 expect(scoped.rows[0].call.oi).toBe(80000);expect(scoped.rows[0].put.oi).toBe(60000);
 expect(scoped.rows[0].call.volume).toBe(14);expect(scoped.rows[0].call.gamma).toBe(40);
 expect(scopeContractReport(report,[]).rows).toEqual([]);
});
