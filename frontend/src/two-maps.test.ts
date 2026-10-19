import {describe,it,expect} from 'vitest';
import {mapGroups,newSelection,twoMapsPanel,usableReport,validCell,type FlowCell,type FlowMapReport} from './two-maps';
import type {LiveState} from './live';
import {tradingMapBars,heatmapMinuteRect} from './two-maps-chart';
const t=Date.parse('2026-10-16T14:00:00Z');
const cell=(patch:Partial<FlowCell>={}):FlowCell=>({ticker:'O:SPY261016C00608000',expiration:'2026-10-16',kind:'call',strike:608,startMs:t,endMs:t+60000,category:'single',contracts:1250,executions:18,verifiedPremium:187500,premium:187500,missingPremiumExecutions:0,...patch});
const report=(cells=[cell()]):FlowMapReport=>({method:'option-execution-heatmap-v1',symbol:'SPY',date:'2026-10-16',asofMs:t+1000,connected:true,firstReceivedMs:t,lastReceivedMs:t,population:'Muestra observada',status:'partial_observed',receptionCoveragePct:null,cells,diagnostics:{}});
const state=(r=report())=>({symbol:'SPY',publishedMs:t+1000,options:{phase:'streaming'},flow:{heatmap:r},ticks:{SPY:{price:608.2}}} as unknown as LiveState);
describe('two maps quantitative boundaries',()=>{
 it('accepts only the identified provider method and selected underlying',()=>{
  expect(usableReport(report(),'SPY',t+1000)).toBe(true);
  for(const r of [{...report(),symbol:'QQQ'},{...report(),method:'made-up'},{...report(),asofMs:t+2000}])expect(usableReport(r,'SPY',t+1000)).toBe(false);
 });
 it.each([{contracts:NaN},{contracts:-1},{contracts:.5},{executions:Infinity},{strike:0},{startMs:t+1},{endMs:t+2},{category:'unknown'},{premium:0},{missingPremiumExecutions:1}])('rejects malformed cell %j',patch=>expect(validCell(cell(patch),t+1000)).toBe(false));
 it('keeps unknown multiplier premium missing without discarding volume',()=>{
  const c=cell({premium:null,verifiedPremium:0,missingPremiumExecutions:18});expect(validCell(c,t+1000)).toBe(true);
  expect(mapGroups(report([c]),newSelection('SPY')).groups[0].premium).toBeNull();
 });
 it('adds compatible categories, never interprets premium as net buying',()=>{
  const g=mapGroups(report([cell(),cell({category:'complex',contracts:10,executions:1,verifiedPremium:100,premium:100})]),newSelection('SPY')).groups[0];
  expect(g.contracts).toBe(1260);expect(g.premium).toBe(187600);
 });
 it('refuses distinct contracts at identical strike and time',()=>{
  const result=mapGroups(report([cell(),cell({ticker:'O:SPY1261016C00608000',category:'complex'})]),newSelection('SPY'));
  expect(result.groups).toHaveLength(0);expect(result.rejected).toBe(1);
 });
 it('does not double count duplicate cell rows',()=>{const r=mapGroups(report([cell(),cell()]),newSelection('SPY'));expect(r.groups[0].contracts).toBe(1250);expect(r.rejected).toBe(1);});
 it('separates expirations and exposes category filtering',()=>{
  const r=report([cell(),cell({expiration:'2026-10-19',contracts:1}),cell({category:'complex',contracts:2})]);
  expect(mapGroups(r,{...newSelection('SPY'),expiry:'2026-10-19'}).groups[0].contracts).toBe(1);
  expect(mapGroups(r,{...newSelection('SPY'),category:'single'}).groups[0].contracts).toBe(1250);
 });
 it('excludes old sessions and observations outside thirty minute display',()=>{const r=report([cell({startMs:t-30*60000,endMs:t-29*60000})]);expect(mapGroups(r,newSelection('SPY')).groups).toHaveLength(0);expect(validCell(cell({startMs:t-86400000,endMs:t-86400000+60000}),t)).toBe(false);});
 it('renders two panels with one scale and unknown cell hatch',()=>{
  const html=twoMapsPanel(state(),'SPY',true,t+1000,{...newSelection('SPY'),view:'flow',strike:608,minute:t});
  expect(html).toContain('CALLS');expect(html).toContain('PUTS');expect(html).toContain('escala lineal común');expect(html).toContain('sin dato verificable, no cero');expect(html).toContain('187.500');expect(html).toContain('sin clasificar');
 });
 it('does not replace unavailable ledger with recent trade lists or demo numbers',()=>{
  const html=twoMapsPanel(undefined,'SPY',false,t,{...newSelection('SPY'),view:'flow'});expect(html).toContain('ledger de ejecuciones');expect(html).not.toContain('187.500');
 });
 it('marks depth unavailable rather than inventing resting or removed orders',()=>{
  const html=twoMapsPanel(state(),'SPY',true,t+1000,newSelection('SPY'));expect(html).toContain('Massive no ofrece Level 2');expect(html).toContain('NO EVALUABLES');
 });
 it('shows only structural levels in the public web view, without execution heat',()=>{
  const html=twoMapsPanel(state(),'SPY',false,t+1000,{...newSelection('SPY'),view:'flow'},undefined,true);
  expect(html).toContain('Mapa de niveles');
  expect(html).not.toContain('Flujo por strike');
  expect(html).not.toContain('Calor =');
  expect(html.toLowerCase()).not.toContain('calor');
  expect(html).not.toContain('data-map-metric');
  expect(html).not.toContain('187.500');
 });
 it('does not fabricate zero PUT volume when only CALL executions were observed',()=>{
  const html=twoMapsPanel(state(),'SPY',true,t+1000,{...newSelection('SPY'),view:'levels'});expect(html).toContain('<span>PUT —</span>');expect(html).toContain('último puede estar en curso');
 });
 it('preserves historical snapshot clocks and never labels delayed data live',()=>{
  const html=twoMapsPanel(state(),'SPY',true,t+3600000,{...newSelection('SPY'),view:'flow'});expect(html).toContain('CAPTURA HISTÓRICA');expect(html).toContain('10:00');
 });
 it('escapes user-facing identity text',()=>expect(twoMapsPanel(undefined,'<script>',false,t)).toContain('&lt;script&gt;'));
 it('preserves missing minutes and excludes invalid or future OHLC rather than fabricating candles',()=>{
  const bar={open:607,high:609,low:606,close:608,volume:20};
  const s={...state(),chartHistory:{minutes:[{...bar,timestampMs:t-180000},{...bar,timestampMs:t-60000},{...bar,timestampMs:t+60000},{...bar,timestampMs:t-120000,high:605}],sessions:[]}} as unknown as LiveState;
  const bars=tradingMapBars(s,t+1000);expect(bars.map(b=>b.timestampMs)).toEqual([t-180000,t-60000]);
  expect(tradingMapBars({...s,chartHistory:{minutes:[{timestampMs:t,close:608}]}} as unknown as LiveState,t)).toEqual([]);
 });
 it('positions each original minute separately when candle interval is five minutes',()=>{
  const rects=Array.from({length:5},(_,i)=>heatmapMinuteRect(t+i*60000,5,100,50));
  expect(rects.map(r=>r.left)).toEqual([75,85,95,105,115]);expect(rects.every(r=>r.width===10)).toBe(true);
 });
 it('aggregates only actual OHLC and preserves unknown volume',()=>{
  const bars=[{timestampMs:t,open:10,high:13,low:9,close:12,volume:20},{timestampMs:t+60000,open:12,high:15,low:11,close:14,volume:NaN}];
  const result=tradingMapBars({chartHistory:{minutes:bars}} as unknown as LiveState,t+60000,2);
  expect(result).toEqual([{timestampMs:t,open:10,high:15,low:9,close:14,volume:NaN}]);
 });
});
