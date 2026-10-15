import {it,expect} from 'vitest';
import {systemAnalysis,systemAnalysisPanel,analystRequest} from './system-analysis';
import type {LearningReport} from './learning';
import type {DirectionalRoute} from './directional-route';
const now=Date.parse('2026-10-05T15:00:00Z');
const report=()=>({generatedMs:now,observations:120,eligible:100,buildSessions:10,outcomes:[{minutes:5,observed:50,pending:20,missing:10,meanBps:5}],directionalCohorts:[],recent:[]} as unknown as LearningReport);
it('does not invent training, probabilities or forecasts from an aggregate return',()=>{
 const p=systemAnalysis(report(),[],'QQQ',now);
 expect(p.status).toBe('collecting');expect(p.projection).toBeNull();expect(p.external.automaticRuleUpdates).toBe(false);
 expect(systemAnalysisPanel(p)).toContain('no probabilidades futuras');expect(analystRequest(p).instructions).toContain('No inventes');
});
it.each([undefined,now-75001,now+1,NaN])('withdraws stale or absent historical evidence %s',at=>{
 const d=at==null?undefined:{...report(),generatedMs:at};expect(systemAnalysis(d,[],'QQQ',now).status).toBe('waiting');
});
it('uses only valid versioned cohorts and keeps flat observations in the denominator',()=>{
 const d=report();d.directionalCohorts=[{symbol:'QQQ',build:'v1',side:'PUT',minutes:5,observed:40,sessions:10,aligned:20,flat:5,meanDirectionalBps:10}];
 const p=systemAnalysis(d,[],'QQQ',now);expect(p.status).toBe('ready-for-review');expect(p.evidence.find(e=>e.source==='directional-cohort')?.facts.alignmentPct).toBe(50);
 d.directionalCohorts[0].aligned=41;expect(systemAnalysis(d,[],'QQQ',now).status).toBe('collecting');
});
it('retains only a current same-asset conditional projection',()=>{
 const route={version:'directional-route-v1',symbol:'QQQ',at:now,state:'supported',side:'CALL',target:105,stop:99,label:'Escenario',invalidation:'Cierre bajo 99'} as DirectionalRoute;
 expect(systemAnalysis(report(),[],'QQQ',now,route).projection?.target).toBe(105);
 for(const r of [{...route,at:now-3001},{...route,symbol:'SPY'},{...route,state:'invalidated' as const},{...route,target:NaN}])expect(systemAnalysis(report(),[],'QQQ',now,r).projection).toBeNull();
});
it('escapes untrusted version text in the local report',()=>{
 const d=report();d.directionalCohorts=[{symbol:'QQQ',build:'<img src=x>',side:'PUT',minutes:5,observed:1,sessions:1,aligned:0,flat:0,meanDirectionalBps:0}];
 const html=systemAnalysisPanel(systemAnalysis(d,[],'QQQ',now));expect(html).toContain('&lt;img');expect(html).not.toContain('<img');
});
it('keeps analysis bounded and fast without rerunning the trading engines',()=>{
 const d=report();d.directionalCohorts=Array.from({length:500},(_,i)=>({symbol:'QQQ',build:'v'+i,side:'CALL',minutes:5,observed:40,sessions:10,aligned:20,flat:0,meanDirectionalBps:2}));
 const start=performance.now();for(let i=0;i<1000;i++)systemAnalysisPanel(systemAnalysis(d,[],'QQQ',now));
 const ms=(performance.now()-start)/1000;console.log(`Local analyst: ${ms.toFixed(3)} ms/report`);expect(ms).toBeLessThan(5);expect(systemAnalysis(d,[],'QQQ',now).evidence.filter(e=>e.source==='directional-cohort')).toHaveLength(200);
});
