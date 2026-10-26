import {expect,it} from 'vitest';
import {chartAbsorptionView} from './absorption-chart-context';
import {absorptionOverlaySvg} from './execution-absorption-overlay';
import {readExecutionAbsorption} from './execution-absorption-reading';
import type {CvdRead} from './cvd';
import observedPending from './execution-absorption-observed-pending.fixture.json';
const at=Date.parse('2026-10-26T15:00:30Z');
const period={buy:0,sell:20000,unknown:0,delta:-20000,coveragePct:100,sinceMs:at-30000,lastMs:at,changeOne:-20000,changeFive:-20000,gaps:0};
it('replays the actual QQQ receiver observation from 26 October 11:39:08 ET without inventing a positive signal',()=>{
 const r=observedPending as CvdRead,original=JSON.stringify(r),view=chartAbsorptionView(r,'QQQ',true,r.at+300000,r.at);
 expect(view.status).toBe('pending');expect(view.candidates).toEqual([]);
 expect(view.detail).toContain('36.0%');expect(view.detail).toContain('últimos 10 s');expect(view.detail).toContain('mínimo 80%');
 expect(absorptionOverlaySvg(view,()=>({x:100,y:100}),600,300)).toBe('');expect(JSON.stringify(r)).toBe(original);
});
function frame():CvdRead{return {symbol:'QQQ',date:'2026-10-26',at,available:true,regular:{...period},premarket:{...period},rejected:0,executionEvidence:{version:'execution-evidence-v1',status:'observing',observedSinceMs:at-30000,windows:[{seconds:30,startMs:at-30000,endMs:at,complete:true}],absorption:{version:'execution-absorption-v1',status:'possible-absorption',reason:'synthetic renderer regression, not observed in market',candidates:[{price:100,firstMs:at-9000,lastMs:at,aggressiveSide:'sell',absorbingSide:'buy',trades:10,shares:20000,dominancePct:100,classifiedPct:100,activityRatio:20,displayedSizeRatio:200,adverseExcursion:0,allowedExcursion:.02,observationMs:9000}]}}};}
it('retains the exact saved detection at its capture clock and labels it as archived',()=>{
 const r=frame(),before=JSON.stringify(r),now=at+300000;
 expect(readExecutionAbsorption(r,'QQQ',true,now).status).toBe('pending');
 const view=chartAbsorptionView(r,'QQQ',true,now,at+500);
 expect(view.status).toBe('possible');expect(view.label).toContain('captura');expect(view.detail).toContain('no es una lectura');
 expect(view.candidates).toEqual(r.executionEvidence!.absorption.candidates);
 expect(absorptionOverlaySvg(view,()=>({x:515,y:100}),600,300)).toContain('OBS. EN CAPTURA');
 expect(JSON.stringify(r)).toBe(before);
});
it('does not loosen live freshness or allow stale, future, foreign or incomplete saved evidence',()=>{
 expect(chartAbsorptionView(frame(),'QQQ',true,at+3001).status).toBe('pending');
 for(const clock of [at-1,at+3001,at+300001,NaN])expect(chartAbsorptionView(frame(),'QQQ',true,at+300000,clock).status).toBe('pending');
 expect(chartAbsorptionView(frame(),'SPY',true,at+300000,at).status).toBe('pending');
 const r=frame();r.executionEvidence!.windows[0].complete=false;expect(chartAbsorptionView(r,'QQQ',true,at+300000,at).status).toBe('pending');
});
it('reports received-but-insufficient classified coverage instead of falsely claiming 30 seconds are missing',()=>{
 const r=frame(),e=r.executionEvidence!;e.status='waiting-for-trades';e.absorption.status='pending';e.absorption.reason='Insufficient classified volume with visible-size coverage (minimum 80%)';e.absorption.candidates=[];e.windows[0].classifiedPct=30.33;
 const view=readExecutionAbsorption(r,'QQQ',true,at);
 expect(view.status).toBe('pending');expect(view.candidates).toEqual([]);expect(view.detail).toContain('30.3%');expect(view.detail).toContain('mínimo 80%');expect(view.detail).not.toContain('Necesita 30 s');
});
