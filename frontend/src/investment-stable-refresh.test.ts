import {describe,it,expect,vi} from 'vitest';
import {refreshInvestmentRegion} from './investment-stable-refresh';
describe('Stable investment reading',()=>{
 it('does not replace identical generated source despite serialized HTML and opened details',()=>{
  let writes=0;const el={querySelectorAll:()=>[],querySelector:()=>null,set innerHTML(_v:string){writes++;},get innerHTML(){return '<details open="">normalized &amp;</details>';}} as unknown as HTMLElement;
  vi.stubGlobal('window',{scrollX:0,scrollY:500,scrollTo:vi.fn()});
  const html='<details><summary>Evidence</summary></details>';
  expect(refreshInvestmentRegion(el,html,'AAPL')).toBe(true);
  expect(refreshInvestmentRegion(el,html,'AAPL')).toBe(false);expect(writes).toBe(1);vi.unstubAllGlobals();
 });
 it('preserves opened sections and research filters when new data arrives for the same company',()=>{
  const old={open:true,querySelector:()=>({textContent:'Valuation'})},next={open:false,querySelector:()=>({textContent:'Valuation'})};
  const input={value:'regular'};let replaced=false;
  const scrollTo=vi.fn();vi.stubGlobal('window',{scrollX:0,scrollY:800,scrollTo});
  const el={querySelectorAll:()=>[replaced?next:old],querySelector:(selector:string)=>selector==='[data-ir-trade-session]'?input:null,set innerHTML(_v:string){replaced=true;input.value='post';}} as unknown as HTMLElement;
  refreshInvestmentRegion(el,'first','QQQ');next.open=true;input.value='regular';refreshInvestmentRegion(el,'updated','QQQ');
  expect(next.open).toBe(true);expect(input.value).toBe('regular');expect(scrollTo).not.toHaveBeenCalled();vi.unstubAllGlobals();
 });
});
