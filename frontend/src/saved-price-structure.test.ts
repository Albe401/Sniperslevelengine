import {it,expect} from 'vitest';
import {capturePriceStructure,savedPriceStructure} from './saved-price-structure';
import type {LiveState} from './live';
const now=Date.parse('2026-10-24T10:00:00Z');
function state(){const s={symbol:'QQQ',publishedMs:now,chartHistory:{sessions:[],minutes:[]}} as unknown as LiveState&{priceStructureReading?:ReturnType<typeof capturePriceStructure>};s.priceStructureReading=capturePriceStructure(s,'QQQ',now);return s;}
it('keeps all five missing scales explicit when frozen history is absent',()=>{
 const s=state();const r=savedPriceStructure(s,'QQQ',now)!;expect(r).toHaveLength(5);expect(r.every(c=>c.side==='SIN DATOS')).toBe(true);expect(r.map(c=>c.interval)).toEqual([15,60,240,1440,10080]);
});
it.each(['symbol','version','clock','published','futureHistory','hash','count','order','nan','negativeBars','futureClose','brokenPivot'])('rejects %s frozen evidence',kind=>{
 const s=state(),r=s.priceStructureReading!;
 if(kind==='symbol')r.symbol='SPY';if(kind==='version')(r as any).version='unknown';if(kind==='clock')r.capturedMs++;
 if(kind==='published')r.publishedMs--;if(kind==='futureHistory')r.historyAsOfMs=now+1;if(kind==='hash')r.historySha256='bad';
 if(kind==='count')r.cards.pop();if(kind==='order')r.cards.reverse();if(kind==='nan')r.cards[0].atr=NaN;
 if(kind==='negativeBars')r.cards[0].bars=-1;if(kind==='futureClose')r.cards[0].endMs=now+1;if(kind==='brokenPivot')r.cards[0].highs=[null as any];
 expect(savedPriceStructure(s,'QQQ',now)).toBeUndefined();
});
it('never reinterprets an old immutable capture at the browser clock',()=>{
 const s=state();expect(savedPriceStructure(s,'QQQ',now)).toBeDefined();expect(savedPriceStructure(s,'QQQ',now+300000)).toBeUndefined();
});
