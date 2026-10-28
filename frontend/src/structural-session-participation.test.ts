import {describe,it,expect} from 'vitest';
import type {StructuralMapContext} from './structural-map-context';
import type {FiveMinuteStructureCard} from './price-structure';
import type {SessionStructurePair} from './session-structure-pair';
import {readSessionParticipation,sessionParticipationDetails} from './structural-session-participation';
const now=Date.parse('2026-10-27T11:00:00-04:00');
function sample(){const five={sourceSymbol:'QQQ',sourcePublishedMs:now}as FiveMinuteStructureCard,pair={symbol:'QQQ',five,availability:'current',side:'CALL'}as SessionStructurePair;
 const activity={status:'available',freshness:'current',intervalMinutes:5,observedMs:now,signal:.5,classifiedCoveragePct:90,temporalCoverage:'complete',reason:'Tramo observado'};
 const c={symbol:'QQQ',atMs:now,framePublishedMs:now,frame:{status:'available',freshness:'current'},activity:{cvd:{...activity},flow:{...activity,status:'partial',temporalCoverage:'not-proven'}},options:{source:'Massive',status:'partial',id:'g',observedMs:now,net:-100,unit:'USD-per-1pct',countCoveragePct:80,reason:'Subtotal'}}as unknown as StructuralMapContext;return{c,five,pair};}
describe('qualified participation without extra directional votes',()=>{
 it('keeps each measurement separate and GEX context-only',()=>{const{c,five,pair}=sample(),before=JSON.stringify(c),r=readSessionParticipation(c,five,pair);expect(r.cvd.state).toBe('acompaña');expect(r.flow.state).toBe('contexto parcial');expect(r.gex.state).toBe('parcial');expect(r.side).toBe('CALL');expect(r.probability).toBeNull();expect(r.entryAuthorized).toBe(false);expect(JSON.stringify(c)).toBe(before);});
 it('opposing or neutral CVD does not rewrite structure',()=>{const{c,five,pair}=sample();c.activity.cvd.signal=-.5;expect(readSessionParticipation(c,five,pair).cvd.state).toBe('contradice');c.activity.cvd.signal=0;expect(readSessionParticipation(c,five,pair).cvd.state).toBe('neutral');});
 it.each(['stale','future','coverage','incomplete','foreign']as const)('keeps %s evidence pending',kind=>{const{c,five,pair}=sample();if(kind==='stale')c.activity.cvd.observedMs=now-3001;if(kind==='future')c.activity.cvd.observedMs=now+1;if(kind==='coverage')c.activity.cvd.classifiedCoveragePct=69.99;if(kind==='incomplete')c.activity.cvd.temporalCoverage='not-proven';if(kind==='foreign')five.sourceSymbol='SPY';expect(readSessionParticipation(c,five,pair).cvd.state).toBe('pendiente');});
 it('never upgrades a snapshot or missing structure into live support',()=>{const{c,five,pair}=sample();c.frame.freshness='historical';const r=readSessionParticipation(c,five,pair);expect(r.availability).toBe('reference');expect(r.cvd.signal).toBeNull();expect(sessionParticipationDetails(r)).toContain('Referencia histórica');});
 it('can show measured CVD with no comparable direction without calling it support',()=>{const{c,five,pair}=sample();pair.side='MIXTA';expect(readSessionParticipation(c,five,pair).cvd.state).toBe('pendiente');expect(readSessionParticipation(c,five,pair).side).toBeNull();});
});
