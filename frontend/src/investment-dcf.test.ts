import {describe,it,expect} from 'vitest';
import {discountedCashFlow,companyDcf,dcfPanel,validAssumptions} from './investment-dcf';
import type {Company} from './investment';
const a={growth:0,discount:10,terminal:0};
describe('DCF',()=>{
it('matches a constant perpetuity without double counting the first five years',()=>{const r=discountedCashFlow(100,10,a)!;expect(r.value).toBeCloseTo(100,8);expect(r.years).toHaveLength(5);expect(r.total).toBeCloseTo(1000,8);});
it('discount and share dilution reduce value; growth increases it',()=>{const base=discountedCashFlow(100,10,a)!.value;expect(discountedCashFlow(100,10,{...a,discount:12})!.value).toBeLessThan(base);expect(discountedCashFlow(100,20,a)!.value).toBeCloseTo(base/2);expect(discountedCashFlow(100,10,{...a,growth:5})!.value).toBeGreaterThan(base);});
it('rejects singular, missing and nonfinite inputs',()=>{expect(validAssumptions({...a,terminal:10})).toBe(false);for(const flow of [0,-1,NaN,Infinity])expect(discountedCashFlow(flow,10,a)).toBeNull();expect(discountedCashFlow(100,0,a)).toBeNull();});
const now=Date.parse('2026-10-10T18:00:00Z');const c={ticker:'TEST',eligible:true,status:'Completo',period:'2026-06-30',asOf:'2026-10-09',priceDate:'2026-10-09',metrics:[{id:'fcf_computed',value:100,source:'OCF - CapEx'},{id:'shares_outstanding',value:10,source:'Reference'},{id:'last_close',value:90}]} as Company;
it('provides ordered scenarios and exposes the approximation in the company',()=>{const d=companyDcf(c,now,a);expect(d.reasons).toEqual([]);expect(d.scenarios[0].result!.value).toBeLessThan(d.scenarios[1].result!.value);expect(d.scenarios[2].result!.value).toBeGreaterThan(d.scenarios[1].result!.value);expect(dcfPanel(c,now)).toContain('no FCFE normalizado');});
it('suppresses values for stale, specialized or incomplete companies',()=>{for(const patch of [{period:'2025-01-01'},{priceDate:'2025-01-01'},{eligible:false},{status:'Especializado'},{metrics:[]}]){const d=companyDcf({...c,...patch} as Company,now,a);expect(d.reasons.length).toBeGreaterThan(0);expect(d.scenarios.every(s=>s.result===null)).toBe(true);}});
});