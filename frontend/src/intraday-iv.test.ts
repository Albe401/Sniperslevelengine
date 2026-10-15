import {expect,it} from 'vitest';
import {IvTracker,rangeNarrative} from './intraday-iv';
it('compares real snapshots rather than repeated UI refreshes',()=>{
 const t=new IvTracker(),a={key:'QQQ:ATM745',at:1000,percent:11.9};
 expect(t.update(a)?.delta).toBeNull();expect(t.update(a)?.delta).toBeNull();
 expect(t.update({...a,at:61000,percent:12.05})?.trend).toBe('EXPANDIENDO');
 expect(t.update({...a,at:121000,percent:12.05})?.trend).toBe('ESTABLE');
 expect(t.update({...a,at:181000,percent:11.9})?.trend).toBe('CONTRAYENDO');
});
it('resets on contract, session, symbol, long gap or missing data and ignores late samples',()=>{
 const t=new IvTracker(),a={key:'QQQ',at:1000,percent:12};t.update(a);
 expect(t.update({...a,at:2000,percent:13})?.delta).toBe(1);
 expect(t.update(a)?.percent).toBe(13);
 expect(t.update({...a,key:'SPY',at:3000})?.delta).toBeNull();
 expect(t.update({...a,key:'SPY',at:1000000})?.delta).toBeNull();
 expect(t.update(undefined)).toBeUndefined();expect(t.update(a)?.delta).toBeNull();
});
it('explains directional remaining range without changing the range calculation',()=>{
 const r={remainingCall:1,remainingPut:8,extensionCall:0,extensionPut:0};
 expect(rangeNarrative('COMPRADORES',r,10)).toContain('casi consumido');
 expect(rangeNarrative('COMPRADORES',{...r,remainingCall:3},10)).toContain('poco recorrido');
 expect(rangeNarrative('VENDEDORES',r,10)).toContain('disponible');
 expect(rangeNarrative('VENDEDORES',{...r,extensionPut:2},10)).toContain('extendido');
 expect(rangeNarrative('COMPRADORES',undefined,10)).toBe('');
});
