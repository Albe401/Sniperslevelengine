import {describe,it,expect} from 'vitest';
import {spxReadingFresh,spxReadingPanel,type SpxReading} from './spx-reading';
import type {SpxSynthetic} from './spx-synthetic';
const now=Date.parse('2026-09-25T14:00:00Z');
const s:SpxSynthetic={phase:'ready',detail:'',contracts:6,estimate:{source:'LEGACY_PUT_CALL_PARITY_APPROXIMATION',approximate:true,usableForAnalysis:true,expiration:'2026-09-25',root:'SPXW',price:6000,candidatePairs:3,pairs:[5995,6000,6005].map(strike=>({strike,synthetic:6000})),oldestQuoteMs:now}};
const r:SpxReading={phase:'ready',detail:'Cadena propia',observedMs:now,referenceMs:now,spot:6000,referenceRoot:'SPXW',referenceExpiration:'2026-09-25',contracts:100,excludedAmContracts:20,horizons:[],oiLevels:[],entryConfirmed:false};
describe('independent SPX reading',()=>{
 it('requires both current own reference and current snapshot',()=>{
  expect(spxReadingFresh(r,s,true,now,now)).toBe(true);
  expect(spxReadingFresh(r,s,false,now,now)).toBe(false);
  expect(spxReadingFresh(r,s,true,now,now+15_001)).toBe(false);
  expect(spxReadingFresh({...r,observedMs:now+1},s,true,now,now)).toBe(false);
  expect(spxReadingFresh({...r,referenceExpiration:'2026-09-28'},s,true,now,now)).toBe(false);
  expect(spxReadingFresh({...r,entryConfirmed:true},s,true,now,now)).toBe(false);
 });
 it('states unavailable index data and does not show stale reference price',()=>{
  expect(spxReadingPanel(r,s,true,now,now)).toContain('100 contratos PM');
  const stale=spxReadingPanel(r,s,false,now,now);
  expect(stale).not.toContain('100 contratos PM');
  expect(stale).toContain('no tiene velas oficiales');
  expect(stale).toContain('No autoriza entradas');
 });
 it('escapes server diagnostics',()=>{
  expect(spxReadingPanel({...r,phase:'waiting',detail:'<script>x</script>'},s,true,now,now)).not.toContain('<script>');
 });
});
