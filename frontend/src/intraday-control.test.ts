import {describe,it,expect} from 'vitest';
import parity from '../../fixtures/intraday-strategy.json';
import {buildIntradayControl,ControlTracker,controlPanel,levelInteraction,priceSignal,ratioSignal} from './intraday-control';
import type {LiveState,Horizon} from './live';
const now=Date.parse('2026-09-25T15:00:00Z');
it('retiene apoyo de griegas internamente sin alterar señal ni mostrarlo',()=>{
 const s=fixture(),before=buildIntradayControl(s,'QQQ',true,now);
 s.greekSupport={symbol:'QQQ',at:now,available:true,comparable:2,interpretation:'INTERNAL_GREEK_ONLY',model:'BSM',contracts:[]};
 const after=buildIntradayControl(s,'QQQ',true,now);
 expect(after.greekSupport?.interpretation).toBe('INTERNAL_GREEK_ONLY');expect(after.net).toBe(before.net);expect(after.coverage).toBe(before.coverage);
 expect(controlPanel(after)).not.toContain('INTERNAL_GREEK_ONLY');
 s.greekSupport.at=now-3001;expect(buildIntradayControl(s,'QQQ',true,now).greekSupport).toBeUndefined();
});
const flow=(minutes:number)=>({minutes,event_count:5,contracts:100,premium:1000,premium_complete:true,bullish_pct:90,bearish_pct:10,direction:'BULLISH FLOW',buckets:{'CALL BUY':{contracts:90,premium:900,trades:4},'PUT BUY':{contracts:10,premium:100,trades:1}}});
const fixture=()=>({symbol:'QQQ',publishedMs:now,startedMs:now-600000,stocks:{phase:'streaming'},options:{phase:'streaming',lastEventMs:now},ticks:{QQQ:{symbol:'QQQ',price:501,tradeMs:now}},metrics:{session:'2026-09-25',phase:'Sesión RTH'},flow:{intraday_windows:[flow(1),flow(5)]}} as unknown as LiveState);
it('integrated control keeps only dealers and volatility risk',()=>{
 const html=controlPanel(buildIntradayControl(fixture(),'QQQ',true,now),true);
 expect(html).toContain('Control de dealers');
 expect(html).not.toContain('DELTA POR STRIKE');
 expect(html).not.toContain('intraday-evidence');
 expect(html).not.toContain('01 · CONTROL AHORA');
 expect(html).not.toContain('02 · NIVEL DECISIVO AHORA');
 expect(html).toContain('Engranaje con lectura rápida');
});
it('keeps dealer markup stable across repeated publications without the removed block',()=>{
 const s=fixture(),baseline=controlPanel(buildIntradayControl(s,'QQQ',true,now),true);
 expect(baseline).not.toContain('dealer-confirmation');
 expect(baseline).toContain('Riesgo de expansión de volatilidad');
 for(let i=1;i<=20;i++){
  s.publishedMs=now+i*100;
  expect(controlPanel(buildIntradayControl(s,'QQQ',true,now+i*100),true)).toBe(baseline);
 }
 expect(controlPanel(buildIntradayControl(s,'QQQ',false,now+2000),true)).not.toContain('dealer-confirmation');
});
describe('Python intraday descriptive port',()=>{
 it('retains the original 24/12/18 weights without filling missing components',()=>{const r=buildIntradayControl(fixture(),'QQQ',true,now);expect(r.coverage).toBe(54);expect(r.strength).toBeCloseTo(80);expect(r.control).toBe('COMPRADORES');});
 it('reads the separate pressure exported by Rust with its six percent weight',()=>{
  const s=fixture();s.optionSnapshotStatus={phase:'ready',observedMs:now,horizons:[{mode:'DAY',structure:{complete:true,quickPressure:63,selected:{},routes:[]}}]} as unknown as LiveState['optionSnapshotStatus'];
  const r=buildIntradayControl(s,'QQQ',true,now);expect(r.coverage).toBe(60);expect(r.components.find(c=>c.name==='PRESIÓN QUICK READ')).toMatchObject({signal:.26,weight:6});
  s.optionSnapshotStatus.observedMs=now-900001;expect(buildIntradayControl(s,'QQQ',true,now).coverage).toBe(54);
 });
 it('does not replace missing 0DTE flow with other expirations',()=>{const s=fixture();s.flow.windows=s.flow.intraday_windows!;s.flow.intraday_windows=[];expect(buildIntradayControl(s,'QQQ',true,now).available).toBe(false);});
 it('rejects another asset, stale publication and disconnect',()=>{for(const r of [buildIntradayControl(fixture(),'SPY',true,now),buildIntradayControl(fixture(),'QQQ',false,now),buildIntradayControl(fixture(),'QQQ',true,now+3001)]){expect(r.coverage).toBe(0);expect(r.control).toBe('ESPERANDO DATOS');}});
 it('rejects expired, future and incomplete flow',()=>{for(const t of [now-15001,now+1001]){const s=fixture();s.options.lastEventMs=t;expect(buildIntradayControl(s,'QQQ',true,now).coverage).toBe(0);}const s=fixture();s.flow.intraday_windows!.forEach(w=>w.premium_complete=false);expect(buildIntradayControl(s,'QQQ',true,now).coverage).toBe(0);});
 it('keeps unknown/zero/invalid ratios unavailable',()=>{expect(ratioSignal(0,0)).toBeNull();expect(ratioSignal(undefined,20)).toBeNull();expect(ratioSignal(NaN,20)).toBeNull();expect(ratioSignal(-1,20)).toBeNull();expect(ratioSignal(0,20)).toBe(-1);});
 it('ports closed-bar direction and VWAP weights',()=>{expect(priceSignal([{timestampMs:now-120000,open:99,close:100,high:101,low:98,volume:1}],100)).toBeCloseTo(.175);expect(priceSignal([],100)).toBeNull();});
 it('recognizes magnet loss with the original 84 score',()=>{const h={structure:{selected:{M1:500},routes:[]}} as unknown as Horizon;const bars=[{timestampMs:now-240000,open:500,close:500.2,low:499.9,high:501,volume:1},{timestampMs:now-120000,open:500.2,close:499,low:498,high:501,volume:1}];expect(levelInteraction(h,bars,499)).toMatchObject({bias:-1,score:84,action:'PERDIDO POR COMPRADORES'});});
 it('uses barrier strength rather than pull score',()=>{const h={structure:{selected:{},routes:[{direction:'CALL',barrier:501,barrierScore:82,score:20}]}} as unknown as Horizon;expect(levelInteraction(h,[],500)).toMatchObject({bias:-1,score:82});});
 it('requires two distinct observations and resets after a gap',()=>{const t=new ControlTracker();const r=buildIntradayControl(fixture(),'QQQ',true,now);r.marketEvidence=undefined;expect(t.update(r,now).transition).toContain('ESTABLECIENDO');expect(t.update(r,now+1).transition).toContain('ESTABLECIENDO');expect(t.update({...r,observation:'new'},now+2).transition).toBe('CONTROL INICIAL CONFIRMADO');expect(t.update({...r,observation:'after gap'},now+20000).transition).toContain('ESTABLECIENDO');});
 it('does not confirm across symbols, sessions or weak coverage',()=>{const t=new ControlTracker();const r=buildIntradayControl(fixture(),'QQQ',true,now);r.marketEvidence=undefined;t.update(r,now);expect(t.update({...r,session:'SPY',observation:'new'},now+1).transition).toContain('ESTABLECIENDO');expect(t.update({...r,coverage:24,observation:'weak'},now+2).transition).toContain('PENDIENTE');});
 it('shows pending instead of equilibrium with no evidence',()=>{const r=buildIntradayControl(undefined,'QQQ',false,now);const html=controlPanel(r);expect(html).toContain('ESPERANDO DATOS');expect(html).toContain('0%');expect(html).not.toContain('EQUILIBRIO');});
});

it('shared production fixture preserves the seven-component model',()=>{const r=buildIntradayControl(parity as unknown as LiveState,'QQQ',true,parity.publishedMs);expect(r.coverage).toBe(100);expect(r.net).toBeCloseTo(0.776380,4);});

it('stock tick alone cannot reconfirm unchanged directional evidence',()=>{
 const s=fixture(),t=new ControlTracker();const a=buildIntradayControl(s,'QQQ',true,now);t.update(a,now);
 s.ticks.QQQ.tradeMs=now+1;s.publishedMs=now+1;
 const b=buildIntradayControl(s,'QQQ',true,now+1);
 expect(b.observation).toBe(a.observation);expect(t.update(b,now+1).transition).not.toContain('CONFIRMADO');
});
it('foreign local volume cannot leak into the selected asset',()=>{
 const s=fixture();s.localVolume={symbol:'SPY',usable:true,last_trade_age_seconds:0,call_contracts_1m:100,put_contracts_1m:0} as LiveState['localVolume'];
 expect(buildIntradayControl(s,'SPY',false,now).coverage).toBe(0);
});
it('nearer barrier is shown before a more distant magnet',()=>{
 const s=fixture();s.optionSnapshotStatus={phase:'ready',observedMs:now,horizons:[{mode:'DAY',structure:{complete:true,selected:{M1:510},routes:[{direction:'CALL',barrier:503,barrierScore:60,structuralReady:true}],callWall:null,putWall:null}}]} as unknown as LiveState['optionSnapshotStatus'];
 expect(buildIntradayControl(s,'QQQ',true,now).next).toBe(503);
});

it('does not confirm control without a recent stock price',()=>{
 const s=fixture();s.ticks.QQQ.tradeMs=now-15001;
 const r=buildIntradayControl(s,'QQQ',true,now);expect(r.control).toBe('EVIDENCIA PARCIAL');expect(r.next).toBeNull();
 const t=new ControlTracker();t.update(r,now);expect(t.update({...r,observation:'new'},now+1).transition).not.toContain('CONFIRMADO');
});
it('invalid flow percentages cannot become extreme directional evidence',()=>{
 const s=fixture();s.flow.intraday_windows!.forEach(w=>w.bullish_pct=200);
 expect(buildIntradayControl(s,'QQQ',true,now).available).toBe(false);
});
it('structural weight alone never certifies control or a target',()=>{
 const s=fixture();s.flow.intraday_windows=[];
 s.optionSnapshotStatus={phase:'ready',observedMs:now,horizons:[{mode:'DAY',structure:{complete:true,quickPressure:100,selected:{M1:510},routes:[]}}]} as unknown as LiveState['optionSnapshotStatus'];
 const r=buildIntradayControl(s,'QQQ',true,now);expect(r.control).toBe('EVIDENCIA PARCIAL');expect(r.next).toBeNull();
});
