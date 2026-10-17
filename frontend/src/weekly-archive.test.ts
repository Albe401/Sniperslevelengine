import {it,expect} from 'vitest';
import fixture from './fixtures/weekly-qqq-close.json';
// @ts-expect-error Runtime shared server module, tested by Vite.
import {assembleWeeklyArchive} from '../../server/weekly-archive.mjs';
import {savedWeeklyRanges} from './saved-weekly-ranges';
import {scannedWeeklyEntry,weeklyRange} from './investment-weekly-ranges';
it('recovers the real QQQ Friday close with archived ATM quotes',()=>{
 const snapshot=assembleWeeklyArchive(fixture.frame,fixture.archived,fixture.capturedMs);
 const entry=scannedWeeklyEntry(snapshot,'QQQ',fixture.capturedMs);
 expect(entry.price).toBe(709.86);expect(entry.days).toBe(7);
 const pair=fixture.archived.contractChain.observation.value.enrichment.details;
 const iv=(pair[0].iv+pair[1].iv)/2;
 const expected=709.86*iv*Math.sqrt(7/365);
 expect(weeklyRange(entry.price,entry.iv,entry.days)?.move).toBeCloseTo(expected,10);
 expect(savedWeeklyRanges('QQQ',snapshot,fixture.capturedMs)).toContain('Movimiento ±');
 expect(snapshot.originalSnapshotAt).toBe(fixture.archived.contractChain.observation.value.at);
});
it('refuses missing closing bars, foreign roots and old archive snapshots',()=>{
 expect(assembleWeeklyArchive({...fixture.frame,chartHistory:{sessions:fixture.frame.chartHistory.sessions,minutes:[]}},fixture.archived,fixture.capturedMs)).toBeUndefined();
 const foreign=structuredClone(fixture.archived);foreign.contractChain.observation.value.symbol='NVDA';
 expect(assembleWeeklyArchive(fixture.frame,foreign,fixture.capturedMs)).toBeUndefined();
 const old=structuredClone(fixture.archived);old.contractChain.observation.value.at-=3600000;
 expect(assembleWeeklyArchive(fixture.frame,old,fixture.capturedMs)).toBeUndefined();
});
