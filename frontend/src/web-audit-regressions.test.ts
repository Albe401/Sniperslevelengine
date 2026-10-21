import {it,expect} from 'vitest';
import {oiStrikeWindow} from './option-oi-heatmap-panel';
import {savedWeeklyRanges} from './saved-weekly-ranges';
// @ts-expect-error Shared runtime module.
import {closedSessionReference,loadWeeklyArchives} from '../../server/weekly-archive.mjs';
import fixture from './fixtures/weekly-qqq-close.json';
it('makes every strike reachable in both directions without passing array bounds',()=>{
 const all=Array.from({length:519},(_,i)=>1000-i),visited=new Set<number>();
 for(let offset=-1000;offset<=1000;offset+=31){const w=oiStrikeWindow(all,722,31,offset);expect(w.start).toBeGreaterThanOrEqual(0);expect(w.start).toBeLessThanOrEqual(w.last);w.strikes.forEach(x=>visited.add(x));}
 expect(visited.size).toBe(all.length);
 expect(oiStrikeWindow([],undefined,31).strikes).toEqual([]);
});
it('keeps earlier weekly references available when the latest close has no chain',async()=>{
 const close=fixture.frame.chartHistory.sessions[0],newer={...close,date:'2026-10-19',openMs:close.openMs+3*86400000,closeMs:close.closeMs+3*86400000};
 const captured=newer.closeMs+86400000;
 const frame={...fixture.frame,symbol:'QQQ',chartHistory:{...fixture.frame.chartHistory,sessions:[close,newer]}};
 let calls=0;const sql={query:async()=>++calls===1?[{frame:{}}]:[{frame:fixture.archived}]};
 const candidates=await loadWeeklyArchives(sql,frame,captured,(x:any)=>x);
 expect(calls).toBe(2);expect(candidates).toHaveLength(1);expect(candidates[0].historicalSession.date).toBe(close.date);
 const invalid={...candidates[0],symbol:'SPY'};
 expect(savedWeeklyRanges('QQQ',invalid,captured,candidates)).toContain('Movimiento ±');
 const active={...frame,chartHistory:{...frame.chartHistory,sessions:[{...newer,openMs:captured-1,closeMs:captured+1}]}};
 expect(await loadWeeklyArchives(sql,active,captured,(x:any)=>x)).toEqual([]);expect(calls).toBe(2);
});
it('uses the actual regular close with retained closing option quotes before opening',()=>{
 const frame={...fixture.frame,symbol:'QQQ',contractChain:structuredClone(fixture.archived.contractChain)};
 frame.contractChain.observation.observedMs=fixture.capturedMs;
 frame.contractChain.observation.value.at=fixture.capturedMs;
 const before=JSON.stringify(frame);
 const report=closedSessionReference(frame,fixture.capturedMs);
 expect(report.spot).toBe(709.86);
 expect(report.priceAt).toBe(fixture.frame.chartHistory.sessions[0].closeMs);
 expect(savedWeeklyRanges('QQQ',report,fixture.capturedMs)).toContain('Movimiento ±');
 expect(JSON.stringify(frame)).toBe(before);
 const wrong=structuredClone(frame);wrong.contractChain.observation.value.symbol='SPY';expect(closedSessionReference(wrong,fixture.capturedMs)).toBeUndefined();
 const future=structuredClone(frame);future.contractChain.observation.observedMs=fixture.capturedMs+1;expect(closedSessionReference(future,fixture.capturedMs)).toBeUndefined();
 const badQuotes=structuredClone(report);badQuotes.enrichment.details.forEach((c:any)=>{c.quoteAt-=86400000;});expect(savedWeeklyRanges('QQQ',badQuotes,fixture.capturedMs)).toContain('Rango pendiente');
});
