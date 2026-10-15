import {expect,it} from 'vitest';
import {readExecutionAbsorption,absorptionForTrade,executionAbsorptionPanel,type ExecutionAbsorptionCandidate} from './execution-absorption-reading';
import {absorptionOverlaySvg} from './execution-absorption-overlay';
import {bigTradeCards} from './big-trade-cards';
import {cvdPanel,type CvdRead,type BigTrade} from './cvd';
const now=Date.parse('2026-10-02T14:00:30Z');
const candidate:ExecutionAbsorptionCandidate={price:100,firstMs:now-9000,lastMs:now,aggressiveSide:'sell',absorbingSide:'buy',trades:10,shares:20000,dominancePct:100,classifiedPct:100,activityRatio:20,displayedSizeRatio:200,adverseExcursion:0,allowedExcursion:.02,observationMs:9000};
const period={buy:0,sell:20000,unknown:0,delta:-20000,coveragePct:100,sinceMs:now-30000,lastMs:now,changeOne:-20000,changeFive:-20000,gaps:0};
function frame():CvdRead{return {symbol:'QQQ',date:'2026-10-02',at:now,available:true,regular:{...period},premarket:{...period},rejected:0,executionEvidence:{version:'execution-evidence-v1',status:'observing',observedSinceMs:now-30000,windows:[{seconds:30,startMs:now-30000,endMs:now,complete:true}],absorption:{version:'execution-absorption-v1',status:'possible-absorption',reason:'test',candidates:[{...candidate}]}}};}
const trade:BigTrade={at:now,price:100,shares:2000,notional:200000,side:-1,sequence:31};
it('enriches sell pressure with buying absorption without reversing CVD or the executed trade',()=>{
 const r=frame(),before=JSON.stringify(r),view=readExecutionAbsorption(r,'QQQ',true,now);
 expect(view.label).toBe('Abs. compradora · $100.00');
 const cvd=cvdPanel(r,'QQQ',true,'regular',now);expect(cvd).toContain('PRESIÓN VENDEDORA');expect(cvd).toContain('Abs. compradora');
 const html=bigTradeCards([trade],31,view.candidates);expect(html).toContain('Venta est. · Abs. compradora');expect(html).toContain('<time>10:00:30</time>');expect(html).toContain('2,000');
 expect(JSON.stringify(r)).toBe(before);expect(trade.side).toBe(-1);
});
it('distinguishes purchasing pressure retained by sellers',()=>{const r=frame();r.executionEvidence!.absorption.candidates[0]={...candidate,aggressiveSide:'buy',absorbingSide:'sell'};expect(readExecutionAbsorption(r,'QQQ',true,now).label).toContain('Abs. vendedora');});
it.each(['foreign','stale','future','date','disconnect','unavailable','incomplete','missing-since','partial','unknown-version','missing-window'])('clears %s evidence',mode=>{
 const r=frame();let symbol='QQQ',active=true,time=now;
 if(mode==='foreign')symbol='IWM';if(mode==='stale')time+=3001;if(mode==='future')r.at+=1;if(mode==='date')r.date='2026-10-01';if(mode==='disconnect')active=false;if(mode==='unavailable')r.available=false;if(mode==='incomplete')r.executionEvidence!.windows[0].complete=false;if(mode==='missing-since')r.executionEvidence!.observedSinceMs=null;if(mode==='partial')r.executionEvidence!.absorption.status='pending';if(mode==='unknown-version')r.executionEvidence!.absorption.version='unknown';if(mode==='missing-window')r.executionEvidence!.windows=[];
 const view=readExecutionAbsorption(r,symbol,active,time);expect(view.status).toBe('pending');expect(view.candidates).toEqual([]);
});
it.each(['dominance','coverage','activity','size','progress','same-side','one-print','expired','future-print','origin','nonfinite'])('rejects malformed candidate %s',mode=>{
 const r=frame(),c=r.executionEvidence!.absorption.candidates[0];
 if(mode==='dominance')c.dominancePct=69;if(mode==='coverage')c.classifiedPct=79;if(mode==='activity')c.activityRatio=1;if(mode==='size')c.displayedSizeRatio=2;if(mode==='progress')c.adverseExcursion=.03;if(mode==='same-side')c.absorbingSide='sell';if(mode==='one-print')c.trades=1;if(mode==='expired')c.lastMs=now-2001;if(mode==='future-print')c.lastMs=now+1;if(mode==='origin')c.firstMs=now-10001;if(mode==='nonfinite')c.shares=NaN;
 expect(readExecutionAbsorption(r,'QQQ',true,now).status).toBe('pending');
});
it('distinguishes an observed lack of candidates from unavailable data',()=>{const r=frame();r.executionEvidence!.absorption.status='observing';r.executionEvidence!.absorption.candidates=[];expect(readExecutionAbsorption(r,'QQQ',true,now).label).toBe('Absorción · sin indicio');r.executionEvidence!.absorption.status='possible-absorption';expect(readExecutionAbsorption(r,'QQQ',true,now).status).toBe('pending');});
it('only annotates trades in the matching price bucket and observed time span',()=>{
 expect(absorptionForTrade(trade,[candidate])).toBe(candidate);
 for(const t of [{...trade,price:100.01},{...trade,price:99.99},{...trade,at:candidate.firstMs-1},{...trade,at:now+1}])expect(absorptionForTrade(t,[candidate])).toBeUndefined();
 expect(bigTradeCards([trade],31,[])).not.toContain('Abs. compradora');
});
it('moves the chart label with coordinates, hides off-screen values and bounds collisions',()=>{
 const view=readExecutionAbsorption(frame(),'QQQ',true,now);
 expect(absorptionOverlaySvg(view,()=>({x:100,y:100}),600,300)).toContain('y1="100"');
 expect(absorptionOverlaySvg(view,()=>({x:200,y:170}),600,300)).toContain('y1="170"');
 for(const p of [null,{x:-1,y:100},{x:550,y:100},{x:100,y:290}])expect(absorptionOverlaySvg(view,()=>p,600,300)).toBe('');
 const stacked={...view,candidates:[candidate,{...candidate,price:100.01},{...candidate,price:101},{...candidate,price:102}]};
 expect((absorptionOverlaySvg(stacked,c=>({x:100,y:c.price===100.01?101:100+(c.price-100)*40}),600,300).match(/<rect/g)??[]).length).toBe(2);
 expect(absorptionOverlaySvg({...view,status:'pending'},()=>({x:100,y:100}),600,300)).toBe('');
});
it('keeps the explanation available without conflating dominant volume with probability',()=>{const html=executionAbsorptionPanel(readExecutionAbsorption(frame(),'QQQ',true,now));expect(html).toContain('presión vendedora 100% del volumen clasificado');expect(html).toContain('no probabilidad');expect(html).toContain('no reconstruye absorción histórica');});
it('reserves the price axis and suppresses labels in panes too narrow to fit them',()=>{
 const view=readExecutionAbsorption(frame(),'QQQ',true,now);
 const svg=absorptionOverlaySvg(view,()=>({x:500,y:100}),600,300);
 const x=Number(svg.match(/<rect x="([\d.]+)"/)?.[1]);
 expect(Number.isFinite(x)).toBe(true);expect(x+164).toBeLessThanOrEqual(600-80);
 expect(absorptionOverlaySvg(view,()=>({x:100,y:100}),260,300)).toBe('');
});
