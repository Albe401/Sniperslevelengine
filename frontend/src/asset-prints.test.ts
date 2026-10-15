import {expect,it} from 'vitest';
import {assetPrints} from './asset-prints';
import type {OffSummary} from './live';
const level=(n:number,prints:number,notional:number)=>({level:n,prints,notional,concentration_pct:20,strength:'MEDIUM',strength_score:50,strength_data_sufficient:true});
const session={date:'2026-09-25',complete:false,top_levels:[level(110,2,500000),level(101,3,200000),level(99,8,300000),level(90,1,100000)],pressure_coverage_pct:0,pressure_pct:90} as OffSummary;
it('selects nearest upper/lower, repeated and highest notional separately',()=>{
 const html=assetPrints(session,100,'Última operación');
 expect(html).toMatch(/Print superior<\/h3><strong>\$101/);
 expect(html).toMatch(/Print inferior<\/h3><strong>\$99/);
 expect(html).toMatch(/Prints repetidos<\/h3><strong>\$99/);
 expect(html).toMatch(/Imán de prints<\/h3><strong>\$110/);
 expect(html).toContain('no concluyente');expect(html).toContain('Cobertura parcial');
});
it('does not infer above/below without a price and escapes references',()=>{
 const html=assetPrints(session,null,'<script>');
 expect(html).toMatch(/Print superior<\/h3><strong>—/);
 expect(html).toContain('&lt;script&gt;');expect(html).not.toContain('<script>');
 expect(assetPrints(undefined,100,'En vivo')).toContain('Esperando precio y niveles');
});
