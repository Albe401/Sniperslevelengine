import {it,expect} from 'vitest';
import {RouteOpportunityRecorder,type RouteOpportunityInput} from './route-opportunity-source';
const open=Date.parse('2026-10-08T09:30:00-04:00');
function frame():RouteOpportunityInput {
 const at=open+11*60000,key='source-sequence',session=`AMZN:2026-10-08:${open}`,barMs=open+10*60000;
 return {symbol:'AMZN',startedMs:open,publishedMs:at,at,market:{symbol:'AMZN',session,key:'market-current',at},bars:[{timestampMs:barMs,open:100.15,high:100.7,low:100.1,close:100.6,volume:150}],strategies:[{
  strategyId:'session-intraday-v1',state:'confirmed',reason:'Existing source confirmed',result:{state:'sustained',direction:1,level:100,target:102,stop:99.97,reason:'Existing sequence',at,key},
  signals:[{id:key+':confirmed',session,availableMs:at,barMs,state:'sustained',direction:1,price:100,text:'source',color:'green',support:'aligned',evidenceKey:'source-cvd-event'}],
  sequence:{identity:session,key,direction:1,phase:'sustained',impulse:{startMs:open+8*60000,endMs:open+9*60000,close:100.4},continuation:{startMs:barMs,endMs:at,close:100.6},anchor:{name:'PIVOTE BAJO',price:100,expiration:'2026-10-08',firstObservedMs:open+7*60000,asofMs:null,source:'session-price'},destination:{name:'PIVOTE ALTO',price:102,expiration:'2026-10-08',firstObservedMs:open+8*60000,asofMs:at,source:'session-price',selectedMs:at}},
  lastClosedBar:{startMs:barMs,endMs:at,close:100.6}
 }]};
}
it('freezes first source geometry and clocks without mutating the input',()=>{
 const input=frame(),before=structuredClone(input),recorder=new RouteOpportunityRecorder();recorder.observe(input);
 const source=recorder.snapshot(input.symbol,input.at)!,event=source.strategies[0].event!;
 expect(input).toEqual(before);expect(Object.isFrozen(input)).toBe(false);expect(Object.isFrozen(event)).toBe(true);expect(Object.isFrozen(event.target)).toBe(true);
 expect(event.origin).toEqual({symbol:input.symbol,session:input.market.session,publishedMs:input.publishedMs,startedMs:input.startedMs,evaluatedMs:input.at,marketEvidenceKey:input.market.key});
 expect(event.signal.evidenceKey).toBe('source-cvd-event');expect(event.signal.evidenceKey).not.toBe(event.origin.marketEvidenceKey);
 expect(event.signal.availableMs).toBe(input.at);expect(event.triggerBar.close).toBe(100.6);expect(event.anchor.price).toBe(100);expect(event.target.selectedMs).toBe(input.at);
 expect(event.expiry).toEqual({clock:'bar-start',lastAllowedBarStartMs:open+18*60000,elapsedMs:600000,comparison:'>'});
});
it('keeps a frozen event separate from blocked state and changed current geometry',()=>{
 const input=frame(),recorder=new RouteOpportunityRecorder();recorder.observe(input);const original=recorder.snapshot(input.symbol,input.at)!.strategies[0].event;
 input.at+=1000;input.publishedMs=input.at;input.market.at=input.at;input.strategies[0].state='blocked';input.strategies[0].reason='Existing source veto';input.strategies[0].result.target=999;
 recorder.observe(input);const next=recorder.snapshot(input.symbol,input.at)!.strategies[0];
 expect(next.current.state).toBe('blocked');expect(next.current.reason).toBe('Existing source veto');expect(next.current.target).toBe(999);expect(next.event).toEqual(original);expect(next.event!.target.price).toBe(102);
});
it('retains frozen geometry for terminal proof although the source clears its live direction and stop',()=>{
 const input=frame(),recorder=new RouteOpportunityRecorder();recorder.observe(input);const event=recorder.snapshot(input.symbol,input.at)!.strategies[0].event;
 input.at+=60000;input.publishedMs=input.at;input.market.at=input.at;const v=input.strategies[0];v.state='invalidated';v.result={...v.result,state:'invalidated',direction:0,target:null,stop:null,reason:'Anchor expired',at:input.at};v.reason='Anchor expired';v.sequence=undefined;v.lastClosedBar={startMs:input.at-60000,endMs:input.at,close:100.1};
 recorder.observe(input);const terminal=recorder.snapshot(input.symbol,input.at)!.strategies[0];expect(terminal.event).toEqual(event);expect(terminal.current.resultState).toBe('invalidated');expect(terminal.current.lastClosedBar!.close).toBe(100.1);expect(terminal.current.reason).toBe('Anchor expired');
});
it.each(['old-event','missing-run','future-publication','unclosed-trigger'])('does not invent original source provenance for %s',kind=>{
 const input=frame();if(kind==='old-event')input.strategies[0].signals[0].availableMs--;if(kind==='missing-run')input.startedMs=null;if(kind==='future-publication')input.publishedMs=input.at+1;if(kind==='unclosed-trigger')input.publishedMs=input.at-1;
 const recorder=new RouteOpportunityRecorder();recorder.observe(input);expect(recorder.snapshot(input.symbol,input.at)!.strategies[0].event).toBeNull();
});
it('preserves missing target timestamps as unknown and keeps strategy identities distinct',()=>{
 const input=frame();input.strategies[0].sequence!.destination=null;const recorder=new RouteOpportunityRecorder();recorder.observe(input);const source=recorder.snapshot(input.symbol,input.at)!;
 expect(source.strategies[0].strategyId).toBe('session-intraday-v1');expect(source.strategies[0].event!.sourceState).toBe('confirmed');expect(source.strategies[0].event!.target).toMatchObject({price:102,selectedMs:null,firstObservedMs:null,asofMs:null});
 const outer=frame();outer.strategies[0].strategyId='important-level-route-v1';outer.strategies[0].state='sustained';outer.strategies[0].signals[0].id=outer.strategies[0].result.key+':sustained:'+outer.at;recorder.observe(outer);expect(recorder.snapshot(outer.symbol,outer.at)!.strategies[0].event!.sourceState).toBe('sustained');
});
it('clears source metadata on a new run and expires snapshot reads without refreshing their clocks',()=>{
 const input=frame(),recorder=new RouteOpportunityRecorder();recorder.observe(input);const old=recorder.snapshot(input.symbol,input.at)!;
 expect(recorder.snapshot(input.symbol,input.at+1)).toBe(old);expect(recorder.snapshot('OTHER',input.at)).toBeUndefined();expect(recorder.snapshot(input.symbol,input.at+3001)).toBeUndefined();
 input.startedMs=input.startedMs!+1;input.market.session+=':new';input.strategies[0].signals=[];recorder.observe(input);expect(recorder.snapshot(input.symbol,input.at)!.strategies[0].event).toBeNull();expect(old.strategies[0].event!.target.price).toBe(102);
});
