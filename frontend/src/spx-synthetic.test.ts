import {describe,it,expect} from 'vitest';
import {spxUsable,spxSyntheticPanel,type SpxSynthetic} from './spx-synthetic';
const now=Date.parse('2026-09-24T15:00:00Z');
const sample=():SpxSynthetic=>({phase:'ready',detail:'Available',contracts:6,estimate:{source:'LEGACY_PUT_CALL_PARITY_APPROXIMATION',approximate:true,usableForAnalysis:true,expiration:'2026-09-25',root:'SPXW',price:6000,candidatePairs:3,pairs:[5995,6000,6005].map(strike=>({strike,synthetic:6000})),oldestQuoteMs:now-1000}});
describe('SPX synthetic panel',()=>{
  it('shows a qualified approximation and revokes it on disconnect or stale clocks',()=>{
    const s=sample();expect(spxUsable(s,true,now,now)).toBe(true);
    expect(spxSyntheticPanel(s,true,now,now)).toContain('≈');
    for(const [connected,published,time] of [[false,now,now],[true,now-3001,now],[true,now+1,now],[true,now+16000,now+16000]] as const){
      expect(spxUsable(s,connected,published,time)).toBe(false);
      expect(spxSyntheticPanel(s,connected,published,time)).not.toContain('≈');
    }
  });
  it('rejects incomplete pairs, wrong provenance and invalid expiration',()=>{
    for(const patch of [{pairs:[]},{source:'INDEX_TICK'},{approximate:false},{usableForAnalysis:false},{root:'QQQ'},{price:NaN},{candidatePairs:2},{oldestQuoteMs:now+1},{expiration:'2026-09-23'},{expiration:'2026-10-05'},{expiration:'2026-09-31'}]){
      const s=sample();Object.assign(s.estimate!,patch);expect(spxUsable(s,true,now,now)).toBe(false);
    }
  });
  it('escapes upstream detail and never substitutes a stale estimate',()=>{
    const s=sample();s.phase='waiting';s.detail='<img src=x onerror=alert(1)>';
    const html=spxSyntheticPanel(s,true,now,now);
    expect(html).toContain('&lt;img');expect(html).not.toContain('<img');expect(html).not.toContain('≈');
    expect(spxSyntheticPanel(undefined,false,undefined,now)).toContain('Sin conexión vigente');
  });
});
