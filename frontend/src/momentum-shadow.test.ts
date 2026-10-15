import {vi,describe,it,expect,beforeEach} from 'vitest';
const h=vi.hoisted(()=>({open:false,now:1000000}));
vi.mock('./continuous-quick-audit',()=>({marketOpen:()=>h.open}));
vi.mock('./intraday-control',()=>({buildIntradayControl:vi.fn()}));
vi.mock('./direction-scanner',()=>({readDirectionScore:()=>({call:80,put:20,side:'CALL',components:[{name:'Prima clasificada 5m',signal:.1+(h.now-1000000)/60000*.03},{name:'Precio y VWAP',signal:.5}]})}));
vi.mock('./route-live',()=>({routeChartReading:()=>({result:{state:'sustained',direction:1,target:103,stop:99}})}));
vi.mock('./intraday-atr',()=>({intradayAtr:()=>({value:1})}));
vi.mock('./candle-series',()=>({currentSessionCandles:()=>Array.from({length:6},(_,i)=>({complete:true,sessionDate:'2026-10-05',timestampMs:h.now-(6-i)*60000,close:100,volume:i===5?120:100}))}));
import {MomentumShadow} from './momentum-shadow';
import type {LiveState} from './live';
const store=new Map<string,string>();
beforeEach(()=>{store.clear();h.open=false;h.now=1000000;vi.stubGlobal('localStorage',{getItem:(k:string)=>store.get(k)??null,setItem:(k:string,v:string)=>store.set(k,v)});});
describe('Armar antes de abrir',()=>{
 it('does not read or fabricate signals before session opens',()=>{const a=new MomentumShadow();a.toggle();for(const h of [7,7.5,8,9])a.observe(undefined,'QQQ',true,h*3600000);expect(a.enabled).toBe(true);expect(a.waiting).toContain('esperando sesión');expect(a.records).toHaveLength(0);});
 it('restores armed state after restart',()=>{const a=new MomentumShadow();a.toggle();const b=new MomentumShadow();expect(b.enabled).toBe(true);expect(b.waiting).toContain('Armado');});
 it('pause remains paused',()=>{const a=new MomentumShadow();a.toggle();a.toggle();expect(new MomentumShadow().enabled).toBe(false);});
 it('storage failure is visible',()=>{vi.stubGlobal('localStorage',{getItem:()=>null,setItem:()=>{throw new Error('full');}});const a=new MomentumShadow();a.toggle();expect(a.error).toContain('No se pudo guardar');});
 it('starts automatically on open, compares and saves subsequent price',()=>{
  const a=new MomentumShadow();a.toggle();a.observe(undefined,'QQQ',true,h.now);h.open=true;
  const state=()=>({symbol:'QQQ',publishedMs:h.now,metrics:{session:'2026-10-05'},ticks:{QQQ:{price:100}},chartHistory:{}} as unknown as LiveState);
  for(let i=0;i<=5;i++){h.now=1000000+i*60000;a.observe(state(),'QQQ',true,h.now);}
  expect(a.current?.read.state).toBe('ACTIVO');expect(a.records[0].outcome).toBe('observed');expect(new MomentumShadow().records).toHaveLength(6);
 });
 it('missing intermediate samples cannot become a measured outcome',()=>{
  const a=new MomentumShadow();a.toggle();h.open=true;
  const state=()=>({symbol:'QQQ',publishedMs:h.now,metrics:{session:'2026-10-05'},ticks:{QQQ:{price:100}},chartHistory:{}} as unknown as LiveState);
  a.observe(state(),'QQQ',true,h.now);h.now+=300000;a.observe(state(),'QQQ',true,h.now);
  expect(a.records[0].outcome).toBe('missing');expect(a.current?.read.score).toBeNull();
 });
});
