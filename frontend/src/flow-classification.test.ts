import {expect,it} from 'vitest';
import {classifiedPremiumPercent,classificationNote} from './flow-classification';
const window=(known:number,unknown:number)=>({event_count:2,premium:known+unknown,premium_complete:true,buckets:{'CALL BUY':{premium:known},'CALL SELL':{premium:0},'PUT BUY':{premium:0},'PUT SELL':{premium:0},UNKNOWN:{premium:unknown}}});
it('distinguishes complete, partial and entirely unclassified premium',()=>{
  expect(classifiedPremiumPercent(window(100,0),true)).toBe(100);
  expect(classifiedPremiumPercent(window(1,99),true)).toBe(1);
  expect(classifiedPremiumPercent(window(0,100),true)).toBe(0);
});
it('never certifies missing, invalid, stale or inconsistent buckets as full coverage',()=>{
  for(const w of [undefined,{...window(100,0),buckets:undefined},{...window(100,0),premium_complete:false},{...window(100,0),event_count:0},{...window(100,0),premium:101},window(NaN,1),window(-1,100),window(Infinity,1),window(0,0)])expect(classifiedPremiumPercent(w,true)).toBeNull();
  const w=window(100,0);delete (w.buckets as Record<string,unknown>).UNKNOWN;
  expect(classifiedPremiumPercent(w,true)).toBeNull();
  expect(classifiedPremiumPercent(window(100,0),false)).toBeNull();
  expect(classifiedPremiumPercent({...window(0,0),premium:1e-12},true)).toBeNull();
});
it('does not round partial classification to 100 percent or a tiny sample to zero',()=>{
  expect(classificationNote(window(99999,1),true)).toContain('>99.9%');
  expect(classificationNote(window(1,99999),true)).toContain('<0.1%');
  expect(classificationNote(window(100,0),true)).toContain('100.0%');
  expect(classificationNote(window(100,0),false)).toBe('Prima clasificada: —');
});
