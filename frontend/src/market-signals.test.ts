import {expect,it} from 'vitest';
import {MarketSignalTracker,chartMarketSignals} from './market-signals';
import type {MarketEvidence} from './market-evidence';
import type {RouteResult} from './route-engine';
import type {LevelEvidence} from './level-evidence';
const e=(at=660000,support:MarketEvidence['support']='aligned')=>({symbol:'QQQ',session:'QQQ:2026-10-05:1',at,key:`${at}:${support}`,price:{ready:true},flow:{at,signal:.5},cvd:{at,signal:support==='opposed'?-.5:support==='aligned'?.5:null},direction:1,support,score:{side:'CALL',call:75}} as MarketEvidence);
const r=(state:RouteResult['state']='recovery',at=660000)=>({state,direction:1,at,key:'route-one',level:100,target:102,stop:99.9,reason:''} as RouteResult);
it('shows conditional level destinations for sustained CALL and PUT without inventing a probability',()=>{
 for(const direction of [1,-1]){const market={...e(),direction,flow:{at:660000,signal:.5*direction},cvd:{at:660000,signal:.5*direction}} as MarketEvidence;
 const route={...r('sustained'),direction,target:direction>0?102:98,stop:direction>0?99.9:100.1};
 const signal=new MarketSignalTracker().read(market,route,600000,true)[0];expect(signal.text).toContain('POSIBLE LLEGADA '+(direction>0?'CALL $102.00':'PUT $98.00'));expect(signal.text).toContain('invalida');expect(signal.text).not.toContain('%');
 }
});
it('does not label an unconfirmed, opposed or inverted route as a possible arrival',()=>{
 for(const route of [r('recovery'),r('test'),{...r('sustained'),target:98},{...r('sustained'),stop:101}])expect(new MarketSignalTracker().read(e(),route,600000,true)[0].text).not.toContain('POSIBLE LLEGADA');
 expect(new MarketSignalTracker().read(e(660000,'opposed'),r('sustained'),600000,true)[0].text).not.toContain('POSIBLE LLEGADA');
});
it('attaches the observed strike profile only to a price event, preserving direction and score',()=>{
 const evidence={at:650000,oi:500,gex:1200,oiShare:30,gammaShare:40,expirations:['2026-10-05']} as LevelEvidence;
 const input={...e(),levels:[{price:100,evidence}]} as MarketEvidence;
 const t=new MarketSignalTracker();expect(t.read(input,r('waiting'),600000,true)).toEqual([]);
 const event=t.read(input,r(),600000,true)[0];expect(event.profile?.oi).toBe(500);expect(event.text).toContain('OI/GEX observado');expect(event.direction).toBe(1);expect(input.score.call).toBe(75);
 expect(new MarketSignalTracker().read({...input,levels:[{price:101,evidence}]} as MarketEvidence,r(),600000,true)[0].profile).toBeUndefined();
 expect(new MarketSignalTracker().read({...input,levels:[{price:100,evidence:{...evidence,at:660001}}]} as MarketEvidence,r(),600000,true)[0].profile).toBeUndefined();
 expect(new MarketSignalTracker().read({...input,levels:[{price:100,evidence:{...evidence,gex:null}}]} as MarketEvidence,r(),600000,true)[0].text).toContain('perfil parcial');
});
it('records each observed stage once; repaints cannot manufacture signals',()=>{
 const t=new MarketSignalTracker();expect(t.read(e(),r(),600000,true)).toHaveLength(1);
 for(let i=0;i<100;i++)expect(t.read(e(660000+i),r(),600000,true)).toHaveLength(1);
 expect(t.read(e(720000),r('test',720000),660000,true)).toHaveLength(2);
 expect(t.read(e(780000),r('sustained',780000),720000,true)).toHaveLength(3);
 const invalid=t.read(e(840000),{...r('invalidated',840000),direction:0},780000,true);
 expect(invalid.at(-1)?.text).toContain('INVALIDADA');expect(invalid.at(-1)?.color).toBe('#ffb564');
});
it('marks disagreement as context, waits for a distinct close, and does not repeatedly add a stage',()=>{
 const t=new MarketSignalTracker();t.read(e(),r(),600000,true);
 expect(t.read(e(660001,'opposed'),r(),600000,true)).toHaveLength(1);
 const events=t.read(e(720000,'opposed'),r('recovery',720000),660000,true);expect(events).toHaveLength(2);expect(events[1].text).toContain('CVD contrario');expect(events[1].state).toBe('context');
 expect(t.read(e(720001,'opposed'),r('recovery',720000),660000,true)).toHaveLength(2);
});
it('clears signals after disconnect, session change, time reversal or incomplete closes',()=>{
 const t=new MarketSignalTracker();t.read(e(),r(),600000,true);expect(t.read(e(),r(),600000,false)).toEqual([]);
 t.read(e(),r(),600000,true);expect(t.read({...e(),session:'SPY:2026-10-05:1'},r('waiting'),600000,true)).toEqual([]);
 t.read(e(),r(),600000,true);expect(t.read(e(599999),r(),600000,true)).toEqual([]);
});
it('places marks at availability on real chart bars, suppressing past, future and foreign overlays',()=>{
 const t=new MarketSignalTracker(),events=t.read(e(),r(),600000,true);
 expect(chartMarketSignals(events,[{timestampMs:600000}],'QQQ',660000,true)).toEqual([]);
 const bars=[{timestampMs:600000},{timestampMs:660000}];expect(chartMarketSignals(events,bars,'QQQ',660000,true)[0].timestampMs).toBe(660000);
 expect(chartMarketSignals(events,bars,'SPY',660000,true)).toEqual([]);expect(chartMarketSignals(events,bars,'QQQ',660000,false)).toEqual([]);expect(chartMarketSignals(events,bars,'QQQ',659999,true)).toEqual([]);
});
it.each([120000,300000,900000])('keeps live events in their containing %i ms chart candle',interval=>{
 const events=new MarketSignalTracker().read(e(),r(),600000,true);
 const start=Math.floor(660000/interval)*interval;
 expect(chartMarketSignals(events,[{timestampMs:start}],'QQQ',660000,true,interval)[0]?.timestampMs).toBe(start);
 expect(chartMarketSignals(events,[{timestampMs:start-interval}],'QQQ',660000,true,interval)).toEqual([]);
});
it('uses amber for missing pressure and opposite flow even when the reported bias is CALL',()=>{
 const partial=e(660000,'pending');expect(new MarketSignalTracker().read(partial,r('sustained'),600000,true)[0].color).toBe('#ffb564');
 const opposite={...e(),flow:{...e().flow,at:660000,signal:-.5}};const mark=new MarketSignalTracker().read(opposite,r('sustained'),600000,true)[0];expect(mark.color).toBe('#ffb564');expect(mark.text).toContain('presión en contra');
 expect(new MarketSignalTracker().read(e(),r('sustained'),600000,true)[0].color).toBe('#25edac');
});

it('keeps incomplete support as a candidate and distinguishes an expired route',()=>{
 const partial=new MarketSignalTracker().read(e(660000,'pending'),r('sustained'),600000,true,{call:null,put:null,state:'Sin probabilidad validada',detail:'',provisional:{call:75,put:25}})[0];
 expect(partial.text).toContain('CANDIDATO CALL');expect(partial.text).not.toContain('POSIBLE LLEGADA');expect(partial.text).toContain('Confluencia 75/100');expect(partial.text).toContain('no es probabilidad');expect(partial.text).not.toContain('75%');
 const expired=new MarketSignalTracker().read(e(),{...r('invalidated'),terminalCause:'expired'},600000,true)[0];
 expect(expired.text).toContain('RUTA CADUCADA');expect(expired.color).toBe('#ffb564');
});
