import {it,expect} from 'vitest';
import {publicationCandleClock} from './publication-candle-clock';
const at=Date.parse('2026-10-08T10:00:00-04:00');
it('caps only candle availability at the original publication, without advancing a future evaluator',()=>{
 expect(publicationCandleClock(at-1,at+1)).toBe(at-1);expect(publicationCandleClock(at+1,at-1)).toBe(at-1);expect(publicationCandleClock(at,at)).toBe(at);
});
it.each([undefined,null,0,-1,NaN,Infinity,at+.1])('fails closed for an invalid publication %s',published=>{
 expect(Number.isNaN(publicationCandleClock(published,at))).toBe(true);
});
it.each([NaN,Infinity,0,-1,at+.1])('fails closed for an invalid evaluation %s',now=>{
 expect(Number.isNaN(publicationCandleClock(at,now))).toBe(true);
});
