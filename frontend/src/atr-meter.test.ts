import {it,expect} from 'vitest';
import {atrMeter} from './atr-meter';
import type {AtrRead} from './intraday-atr';
it.each([null,undefined,NaN,Infinity,-1])('does not fabricate a bar for invalid magnitude %s',value=>{const h=atrMeter(undefined,value);expect(h).toContain('pendiente');expect(h).toContain('width:0.00%');expect(h).not.toContain('NaN');});
it('shows actual percentage beyond the scale without calling it exhaustion',()=>{const h=atrMeter(undefined,2);expect(h).toContain('200%');expect(h).toContain('width:100.00%');expect(h).toContain('no confirma agotamiento');});
it('represents zero and one ATR correctly on the disclosed 150 percent scale',()=>{expect(atrMeter(undefined,0)).toContain('>0%</span>');expect(atrMeter(undefined,1)).toContain('width:66.67%');});
it('uses the shared verified RTH context and distinguishes the five minute bar',()=>{const a={context:{rthRangeDaily:.5}} as AtrRead;expect(atrMeter(a)).toContain('50%');expect(atrMeter(a,1.2,'Última vela / ATR 5 min')).toContain('Última vela / ATR 5 min');});
