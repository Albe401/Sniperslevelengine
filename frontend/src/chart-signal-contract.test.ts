import {describe,it,expect} from 'vitest';
import {CHART_SIGNAL_CONTRACT_VERSION,projectChartSignal,type ChartSignalProposal,type ChartSignalEvent,type ChartSignalRecord} from './chart-signal-contract';
import {ROUTE_BAR_MS,routeSequenceDeadlineMs} from './route-sequence-policy';

// Synthetic contract boundaries only. These cases do not demonstrate trading profitability.
const open=Date.parse('2026-10-26T09:30:00-04:00'),impulse=open+30*ROUTE_BAR_MS,trigger=impulse+2*ROUTE_BAR_MS;
function proposal(side:'CALL'|'PUT'='CALL'):ChartSignalProposal {
 return {identity:{id:'synthetic-episode-1',symbol:'QQQ',sessionKey:'QQQ:2026-10-26:synthetic',strategyId:'session-intraday-v1',sourceEventKey:'synthetic-route-1'},session:{openMs:open,closeMs:open+390*ROUTE_BAR_MS},side,
  impulseCloseMs:impulse,triggerCloseMs:trigger,deadlineMs:routeSequenceDeadlineMs(impulse),sourceDataMs:trigger+500,publishedMs:trigger+750,availableMs:trigger+1000,
  source:{kind:'verified-sequence',state:'confirmed',support:'complete'},entryReference:{price:100,atMs:trigger+250,kind:'observed-underlying-reference-not-fill'},target:{price:side==='CALL'?103:97,name:'synthetic reference',firstAvailableMs:impulse-ROUTE_BAR_MS,selectedMs:trigger},invalidation:{price:side==='CALL'?99:101,method:'one-minute-close'},
  quality:{status:'qualified',reasonCodes:['synthetic-source-qualified']},provenance:{engine:'synthetic-engine',engineVersion:'test-v1',frameId:'synthetic-frame',evidenceKey:'synthetic-evidence',sourceFingerprint:'synthetic-frozen-source',coverage:[{name:'classified-trades',unit:'trades',status:'available',numerator:75,denominator:100}]},reasonCodes:['synthetic-technical-sequence']};
}
function initial(p=proposal(),kind:'candidate'|'confirm'='confirm',id='admit'):ChartSignalEvent{const base={eventId:id,evaluatedMs:p.availableMs+20,proposal:p};return kind==='candidate'?{...base,kind:'candidate'}:{...base,kind:'confirm'};}
function confirmed(p=proposal()):Readonly<ChartSignalRecord>{const r=projectChartSignal(null,initial(p));expect(r.accepted).toBe(true);return r.signal!;}
function close(s:Readonly<ChartSignalRecord>,closeMs:number,price:number,id='close'):Extract<ChartSignalEvent,{kind:'resolve-close'}> {
 return {kind:'resolve-close',eventId:id,identity:{...s.identity},sourceFingerprint:s.proposal.provenance.sourceFingerprint,sourceDataMs:closeMs,publishedMs:closeMs+200,availableMs:closeMs+300,evaluatedMs:closeMs+400,reasonCodes:['synthetic-closed-bar'],bar:{startMs:closeMs-ROUTE_BAR_MS,closeMs,close:price,intervalMs:60000}};
}
function observe(s:Readonly<ChartSignalRecord>,ms=trigger+2000,id='observe'):Extract<ChartSignalEvent,{kind:'observe'}>{return {kind:'observe',eventId:id,identity:{...s.identity},sourceFingerprint:s.proposal.provenance.sourceFingerprint,sourceDataMs:ms,publishedMs:ms+100,availableMs:ms+200,evaluatedMs:ms+300,reasonCodes:['synthetic-observation']};}
function expiry(s:Readonly<ChartSignalRecord>,watermark=s.proposal.deadlineMs):Extract<ChartSignalEvent,{kind:'expire'}> {const at=Math.max(watermark,s.lastAcceptedClock.sourceDataMs);return {kind:'expire',eventId:'expiry',identity:{...s.identity},sourceFingerprint:s.proposal.provenance.sourceFingerprint,sourceDataMs:at,publishedMs:at+1000,availableMs:at+2000,evaluatedMs:Math.max(at+3000,s.lastAcceptedEvaluationMs+1),completeThroughCloseMs:watermark,reasonCodes:['synthetic-complete-through-close']};}
function neutralUntil(s:Readonly<ChartSignalRecord>,exclusiveClose:number):Readonly<ChartSignalRecord>{
 let r=s;const first=s.proposal.session.openMs+(Math.floor((Math.max(s.ticket!.availableMs,s.ticket!.confirmedAtMs)-s.proposal.session.openMs)/ROUTE_BAR_MS)+1)*ROUTE_BAR_MS;
 for(let at=r.lastResolvedCloseMs===null?first:r.lastResolvedCloseMs+ROUTE_BAR_MS;at<exclusiveClose;at+=ROUTE_BAR_MS){const step=projectChartSignal(r,close(r,at,100,'neutral-'+at));expect(step.accepted).toBe(true);r=step.signal!;}return r;
}

describe('chart signal contract v1: explicit observational metadata',()=>{
 it('keeps incomplete preparation as candidate and quality separate from lifecycle',()=>{
  const p=proposal();p.source={kind:'preparation',state:'candidate',support:'missing'};p.quality={status:'blocked',reasonCodes:['missing-target']};p.target=null;p.invalidation=null;
  const r=projectChartSignal(null,initial(p,'candidate'));expect(r.accepted).toBe(true);expect(r.signal?.state).toBe('candidate');expect(r.signal?.ticket).toBeNull();expect(r.signal?.proposal.quality.status).toBe('blocked');
  expect(projectChartSignal(r.signal,initial(p,'confirm','attempt')).reasonCode).toBe('source-not-qualified');
 });
 it.each(['target','invalidation','entryReference'] as const)('does not confirm without explicit %s',field=>{const p=proposal();p[field]=null;expect(projectChartSignal(null,initial(p)).reasonCode).toBe('missing-geometry');});
 it.each(['CALL','PUT'] as const)('freezes signed %s geometry in underlying units, never probability',side=>{
  const r=confirmed(proposal(side));expect(r.version).toBe(CHART_SIGNAL_CONTRACT_VERSION);expect(r.state).toBe('confirmed');expect(r.ticket?.geometry).toEqual({direction:side==='CALL'?1:-1,risk:1,reward:3,rewardRisk:3,unit:'underlying-price-units'});
  for(const v of [r,r.ticket]){expect(v?.entryAuthorized).toBe(false);expect(v?.executionEnabled).toBe(false);expect(v?.probability).toBeNull();expect(v?.fill).toBeNull();}
  expect(Object.isFrozen(r.ticket?.target)).toBe(true);expect(Object.isFrozen(r.history[0].event)).toBe(true);
 });
 it('copies caller data before freezing and preserves source observations',()=>{
  const p=proposal(),r=confirmed(p);p.target!.price=999;p.provenance.coverage[0].numerator=1;
  expect(r.ticket?.target?.price).toBe(103);expect(r.ticket?.provenance.coverage[0].numerator).toBe(75);expect(Object.isFrozen(p)).toBe(false);
 });
 it('does not promote legacy partial/sustained to confirmation',()=>{
  const p=proposal();p.identity.strategyId='important-level-route-v1';p.source={kind:'legacy-partial',state:'sustained',support:'partial'};p.quality.status='partial';
  expect(projectChartSignal(null,initial(p)).reasonCode).toBe('source-not-qualified');
  p.source.kind='verified-sequence';p.source.support='complete';p.quality.status='qualified';expect(projectChartSignal(null,initial(p)).accepted).toBe(true);
 });
 it('requires actual source confirmation and explicit complete support',()=>{
  const p=proposal();p.source.state='sustained';expect(projectChartSignal(null,initial(p)).reasonCode).toBe('source-not-confirmed');
  p.source.state='confirmed';p.source.support='missing';expect(projectChartSignal(null,initial(p)).reasonCode).toBe('source-not-qualified');
 });
 it.each(['partial','unavailable'] as const)('preserves %s coverage without manufacturing confirmation',status=>{
  const p=proposal();p.provenance.coverage=[{name:'trades',unit:'trades',status,numerator:status==='unavailable'?null:3,denominator:status==='unavailable'?null:10}];
  expect(projectChartSignal(null,initial(p,'candidate')).accepted).toBe(true);expect(projectChartSignal(null,initial(p)).reasonCode).toBe('source-not-confirmed');
 });
 it('rejects empty or inconsistent denominator provenance',()=>{
  const p=proposal();p.provenance.coverage=[];expect(projectChartSignal(null,initial(p)).reasonCode).toBe('missing-provenance-or-coverage');
  p.provenance.coverage=[{name:'trades',unit:'trades',status:'available',numerator:11,denominator:10}];expect(projectChartSignal(null,initial(p)).reasonCode).toBe('invalid-coverage');
 });
 it.each(['CALL','PUT'] as const)('rejects wrong-side target or stop in a %s proposal',side=>{
  const p=proposal(side);p.target!.price=side==='CALL'?99:101;expect(projectChartSignal(null,initial(p,'candidate')).reasonCode).toBe('invalid-signed-geometry');
  const q=proposal(side);q.invalidation!.price=100;expect(projectChartSignal(null,initial(q)).reasonCode).toBe('invalid-signed-geometry');
 });
 it('rejects source/publication/availability/evaluation lookahead',()=>{
  const p=proposal();p.publishedMs=p.sourceDataMs-1;expect(projectChartSignal(null,initial(p)).reasonCode).toBe('non-causal-clock');
  const q=proposal(),e=initial(q);e.evaluatedMs=q.availableMs-1;expect(projectChartSignal(null,e).reasonCode).toBe('non-causal-clock');
 });
 it('rejects target or entry references not yet available',()=>{
  const p=proposal();p.target!.firstAvailableMs=p.triggerCloseMs+1;expect(projectChartSignal(null,initial(p)).reasonCode).toBe('unavailable-target');
  const q=proposal();q.entryReference!.atMs=q.sourceDataMs+1;expect(projectChartSignal(null,initial(q)).reasonCode).toBe('invalid-entry-reference');
 });
 it('allows candidate to acquire geometry later, retaining the earlier incomplete proposal',()=>{
  const p=proposal();p.target=null;const a=projectChartSignal(null,initial(p,'candidate'));const b=projectChartSignal(a.signal,initial(proposal(),'confirm','confirm'));
  expect(b.accepted).toBe(true);expect(b.signal?.history).toHaveLength(2);expect((b.signal?.history[0].event as {proposal:ChartSignalProposal}).proposal.target).toBeNull();expect(b.signal?.ticket?.target?.price).toBe(103);
 });
 it('rejects identity/session/side/impulse changes and deadline extension',()=>{
  const p=proposal(),r=projectChartSignal(null,initial(p,'candidate')).signal!;
  const q=proposal();q.identity.symbol='SPY';expect(projectChartSignal(r,initial(q,'confirm','foreign')).reasonCode).toBe('foreign-signal');
  const x=proposal();x.side='PUT';x.entryReference=null;x.target=null;x.invalidation=null;expect(projectChartSignal(r,initial(x,'candidate','side')).reasonCode).toBe('episode-revised');
  const y=proposal();y.deadlineMs+=ROUTE_BAR_MS;expect(projectChartSignal(r,initial(y,'confirm','extend')).reasonCode).toBe('inconsistent-deadline');
 });
 it('does not confirm at or after deadline, including delayed retrospective admission',()=>{
  const p=proposal();p.availableMs=p.deadlineMs;expect(projectChartSignal(null,initial(p)).reasonCode).toBe('confirmation-deadline');
  const q=proposal(),e=initial(q);e.evaluatedMs=q.deadlineMs;expect(projectChartSignal(null,e).reasonCode).toBe('confirmation-deadline');
  e.evaluatedMs=q.deadlineMs-1;expect(projectChartSignal(null,e).accepted).toBe(true);
 });
 it('caps the fixed deadline at the supplied session close',()=>{
  const p=proposal();p.session.closeMs=impulse+4*ROUTE_BAR_MS;p.deadlineMs=p.session.closeMs;expect(confirmed(p).ticket?.deadlineMs).toBe(p.session.closeMs);
 });
 it('makes active an observed scenario with the same immutable ticket',()=>{
  const r=confirmed(),a=projectChartSignal(r,observe(r));expect(a.accepted).toBe(true);expect(a.signal?.state).toBe('active');expect(a.signal?.ticket).toBe(r.ticket);expect(a.signal?.executionEnabled).toBe(false);
 });
 it('cannot mark a stale observation active after its evaluation deadline',()=>{const r=confirmed(),e=observe(r);e.evaluatedMs=r.ticket!.deadlineMs;expect(projectChartSignal(r,e).reasonCode).toBe('observation-after-deadline');});
 it('preserves duplicate and revision audit attempts without changing a frozen ticket',()=>{
  const r=confirmed(),dup=projectChartSignal(r,initial());expect(dup.reasonCode).toBe('duplicate-event');expect(dup.audit.disposition).toBe('duplicate');expect(dup.signal?.history).toHaveLength(2);
  const e=initial();e.evaluatedMs++;const rev=projectChartSignal(dup.signal,e);expect(rev.reasonCode).toBe('event-revised');expect(rev.signal?.ticket).toBe(r.ticket);expect(rev.signal?.history).toHaveLength(3);
  const q=proposal();q.target!.price=104;expect(projectChartSignal(r,initial(q,'confirm','new-id')).reasonCode).toBe('immutable-ticket-revision');
 });
 it('rejects changed frozen source and backward clocks while retaining previous evidence',()=>{
  const r=confirmed(),e=observe(r);if(e.kind==='observe')e.sourceFingerprint='revised-source';expect(projectChartSignal(r,e).reasonCode).toBe('frozen-source-revised');
  const a=projectChartSignal(r,observe(r)).signal!,back=observe(a,trigger+1000,'back');expect(projectChartSignal(a,back).reasonCode).toBe('backward-evaluation');
  const b=observe(a,trigger+1000,'back-source');b.evaluatedMs=trigger+5000;expect(projectChartSignal(a,b).reasonCode).toBe('backdated-source');
 });
 it.each(['CALL','PUT'] as const)('resolves %s target/invalidation from future one-minute closes',side=>{
  const r=confirmed(proposal(side)),at=trigger+ROUTE_BAR_MS;
  expect(projectChartSignal(r,close(r,at,side==='CALL'?103:97)).signal?.state).toBe('reached');
  expect(projectChartSignal(r,close(r,at,side==='CALL'?98.99:101.01)).signal?.state).toBe('invalidated');
  expect(projectChartSignal(r,close(r,at,100)).signal?.state).toBe('active');
 });
 it.each(['CALL','PUT'] as const)('preserves %s at the exact stop and invalidates only beyond it',side=>{const r=confirmed(proposal(side)),at=trigger+ROUTE_BAR_MS;expect(projectChartSignal(r,close(r,at,r.ticket!.invalidation!.price)).signal?.state).toBe('active');expect(projectChartSignal(r,close(r,at,side==='CALL'?98.99:101.01)).signal?.state).toBe('invalidated');});
 it.each([-1,0,1])('deadline offset %i minutes resolves by close before expiry, then expires',offset=>{
  const initial=confirmed(),at=initial.ticket!.deadlineMs+offset*ROUTE_BAR_MS,r=neutralUntil(initial,at),a=projectChartSignal(r,close(r,at,103));expect(a.accepted).toBe(true);expect(a.signal?.state).toBe(offset<=0?'reached':'expired');
 });
 it('rejects a skipped minute that could conceal an earlier invalidation or target',()=>{
  const r=confirmed(),gap=projectChartSignal(r,close(r,trigger+2*ROUTE_BAR_MS,103));expect(gap.reasonCode).toBe('missing-close-continuity');expect(gap.signal?.state).toBe('confirmed');expect(gap.signal?.ticket).toBe(r.ticket);
  expect(projectChartSignal(r,close(r,r.ticket!.deadlineMs+ROUTE_BAR_MS,103)).reasonCode).toBe('missing-close-continuity');
  const first=projectChartSignal(gap.signal,close(r,trigger+ROUTE_BAR_MS,98.99,'earlier-stop'));expect(first.signal?.state).toBe('invalidated');
 });
 it('resolves an exact-deadline close arriving after the clock deadline when continuity is proven',()=>{
  const initial=confirmed(),r=neutralUntil(initial,initial.ticket!.deadlineMs),e=close(r,r.ticket!.deadlineMs,103);expect(e.availableMs).toBeGreaterThan(r.ticket!.deadlineMs);expect(projectChartSignal(r,e).signal?.state).toBe('reached');
 });
 it('rejects historic/unclosed/wrong-duration/unaligned bars without using wicks or fills',()=>{
  const r=confirmed();expect(projectChartSignal(r,close(r,trigger,103)).reasonCode).toBe('backward-evaluation');
  const e=close(r,trigger+ROUTE_BAR_MS,103);if(e.kind==='resolve-close')e.bar.intervalMs=300000 as 60000;expect(projectChartSignal(r,e).reasonCode).toBe('unverified-one-minute-close');
  expect(projectChartSignal(r,close(r,trigger+ROUTE_BAR_MS+1,103)).reasonCode).toBe('unverified-one-minute-close');
  const q=close(r,trigger+ROUTE_BAR_MS,103);q.sourceDataMs--;expect(projectChartSignal(r,q).reasonCode).toBe('unverified-one-minute-close');
 });
 it('requires an explicit complete-through watermark to expire without a later bar',()=>{
  const r=confirmed();expect(projectChartSignal(r,expiry(r,r.ticket!.deadlineMs-ROUTE_BAR_MS)).reasonCode).toBe('unverified-expiry-watermark');
  const e=expiry(r);e.evaluatedMs=r.ticket!.deadlineMs;e.availableMs=r.ticket!.deadlineMs;e.publishedMs=r.ticket!.deadlineMs;expect(projectChartSignal(r,e).reasonCode).toBe('unverified-expiry-watermark');
  expect(projectChartSignal(r,expiry(r)).reasonCode).toBe('expiry-without-complete-close-coverage');
  const complete=neutralUntil(r,r.ticket!.deadlineMs+ROUTE_BAR_MS);expect(projectChartSignal(complete,expiry(complete)).signal?.state).toBe('expired');
 });
 it('can expire an incomplete candidate by watermark without claiming a trade outcome',()=>{const p=proposal();p.target=null;const r=projectChartSignal(null,initial(p,'candidate')).signal!;expect(projectChartSignal(r,expiry(r)).signal?.state).toBe('expired');expect(projectChartSignal(r,expiry(r)).signal?.ticket).toBeNull();});
 it('allows expiry when no aligned close is eligible before a supplied non-aligned session end',()=>{const p=proposal();p.session.closeMs=impulse+4*ROUTE_BAR_MS+30000;p.deadlineMs=p.session.closeMs;const e=initial(p);e.evaluatedMs=p.deadlineMs-1000;const r=projectChartSignal(null,e).signal!;expect(projectChartSignal(r,expiry(r)).signal?.state).toBe('expired');});
 it('anchors eligible closes strictly after actual confirmation as well as source availability',()=>{
  const p=proposal(),e=initial(p);e.evaluatedMs=trigger+ROUTE_BAR_MS+1000;const r=projectChartSignal(null,e).signal!,old=close(r,trigger+ROUTE_BAR_MS,103);old.evaluatedMs=e.evaluatedMs+1000;
  expect(projectChartSignal(r,old).reasonCode).toBe('missing-close-continuity');expect(projectChartSignal(r,close(r,trigger+2*ROUTE_BAR_MS,100)).accepted).toBe(true);
 });
 it.each(['reached','invalidated','expired'] as const)('never resurrects a terminal %s episode',state=>{
  const r=confirmed(),complete=state==='expired'?neutralUntil(r,r.ticket!.deadlineMs+ROUTE_BAR_MS):r,terminal=state==='expired'?projectChartSignal(complete,expiry(complete)).signal!:projectChartSignal(r,close(r,trigger+ROUTE_BAR_MS,state==='reached'?103:98.99)).signal!;
  const later=close(terminal,terminal.proposal.deadlineMs+2*ROUTE_BAR_MS,103,'resurrect');expect(projectChartSignal(terminal,later).reasonCode).toBe('terminal-state');expect(projectChartSignal(terminal,later).signal?.state).toBe(state);
 });
 it('cannot observe or resolve an incomplete candidate as a position',()=>{
  const p=proposal();p.target=null;const r=projectChartSignal(null,initial(p,'candidate')).signal!;expect(projectChartSignal(r,observe(r)).reasonCode).toBe('not-confirmed');expect(projectChartSignal(r,close(r,trigger+ROUTE_BAR_MS,103)).reasonCode).toBe('not-confirmed');
 });
});
