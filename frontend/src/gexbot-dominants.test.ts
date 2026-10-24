import {it,expect} from 'vitest';
import {gexbotMarks,type GexbotDominants} from './gexbot-dominants';
const captured=Date.parse('2026-10-20T14:05:00Z');
const reading:GexbotDominants={source:'Gexbot Classic',symbol:'QQQ',category:'gex_full',generatedMs:captured-60000,positiveOi:718,negativeOi:700,positiveVolume:725,negativeVolume:718,zeroGamma:722.5};
it('shows four Gexbot OI/volume dominants and the independent zero-gamma reference',()=>{
 const marks=gexbotMarks(reading,'QQQ',captured,captured+300000);
 expect(marks.map(m=>[m.price,m.label,m.side])).toEqual([
  [725,'GEX + VOL','positive'],[718,'GEX − VOL','negative'],
  [718,'GEX + OI','positive'],[700,'GEX − OI','negative'],[722.5,'ZERO GAMMA VOL','zero'],
 ]);
});
it('hides foreign, future and stale provider observations',()=>{
 expect(gexbotMarks(reading,'SPY',captured,captured+300000)).toEqual([]);
 expect(gexbotMarks({...reading,generatedMs:captured+1},'QQQ',captured,captured+300000)).toEqual([]);
 expect(gexbotMarks(reading,'QQQ',captured,captured+1200001)).toEqual([]);
 expect(gexbotMarks({...reading,positiveVolume:NaN},'QQQ',captured,captured+300000)).toHaveLength(4);
});

it('retains all available independent references when one is absent, preserving source time',()=>{const r=gexbotMarks({...reading,negativeOi:null},'QQQ',captured,captured);expect(r).toHaveLength(4);expect(r.some(v=>v.label==='GEX + OI')).toBe(true);expect(gexbotMarks(reading,'QQQ',captured,NaN)).toEqual([]);});
