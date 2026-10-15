import {it,expect} from 'vitest';
import {researchWindows,researchMetrics,investigateRouteModels} from './route-model-research';
it('creates three nonoverlapping future blocks and purges crossing outcomes without modifying input',()=>{
 const rows=Array.from({length:120},(_,i)=>({at:1000+i*1000,deadline:1500+i*1000,session:String(i).padStart(3,'0')}));rows[59].deadline=rows[60].at+1;
 const before=JSON.stringify(rows),w=researchWindows(rows);expect(w).toHaveLength(3);expect(w.map(v=>v.future.length)).toEqual([20,20,20]);expect(w[0].training.some(r=>r.session==='059')).toBe(false);expect(w.every(v=>v.training.every(r=>r.deadline<v.first))).toBe(true);expect(w[0].future.at(-1)!.at).toBeLessThan(w[1].first);expect(JSON.stringify(rows)).toBe(before);
});
it('measures numerical improvement and exposes bad calibration hidden by a pooled average',()=>{
 const p=Array.from({length:200},(_,i)=>i<100?[.8,.2]:[.2,.8]),y=Array.from({length:200},(_,i)=>i<100?0:1),m=researchMetrics(p,y,[.5,.5]);
 expect(m.improvesBoth).toBe(true);expect(m.brier).toBeCloseTo(.04);expect(m.bands.find(b=>b.label===1&&b.bin===8)?.observed).toBe(1);
 expect(()=>researchMetrics([[NaN,1]],[1],[.5,.5])).toThrow();expect(()=>researchMetrics([[.5,.5]],[2],[.5,.5])).toThrow();
});
it('blocks empty histories, never enables execution and does not call synthetic performance a market improvement',()=>{for(const kind of ['quant','arrival'] as const){const r=investigateRouteModels({},'QQQ',kind);expect(r.marketImprovementProven).toBe(false);expect(r.automaticPublication).toBe(false);expect(r.executionEnabled).toBe(false);expect(r.experiments).toEqual([]);}});
