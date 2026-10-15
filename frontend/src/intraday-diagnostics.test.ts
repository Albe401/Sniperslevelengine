import {describe,it,expect} from 'vitest';
import {IntradayDiagnostics,type DiagnosticInput} from './intraday-diagnostics';
const now=Date.parse('2026-09-25T15:00:00Z');
const sample=():DiagnosticInput=>({session:'QQQ:today:1',ready:true,at:now,spot:100,target:105,net:.6,flow:.7,bars:[],levels:[{key:'call:105:expiry',name:'CALL WALL',level:105,score:60,at:now}]});
describe('Audited intraday diagnostics',()=>{
 it('tracks a lower target and weakening selling pressure symmetrically',()=>{
  const bars=[103,102,101,100.9].map((close,i)=>({timestampMs:now-(4-i)*120000,open:close+.2,close,high:close+.3,low:close-.3,volume:100}));
  const x={...sample(),net:-.6,flow:-.7,spot:101,target:95,bars};
  const t=new IntradayDiagnostics();expect(t.update(x).exhaustion).toContain('PARCIALES');
  const next=t.update({...x,at:now+10000,spot:100.9,flow:-.4});
  expect(next.approach).toContain('ACERCÁNDOSE');expect(next.exhaustion).toContain('POSIBLE AGOTAMIENTO');
  expect(t.update({...x,at:now+11000,spot:102}).approach).toContain('ALEJÁNDOSE');
 });
 it('does not call opposing flow exhaustion while price keeps progressing',()=>{
  for(const direction of [1,-1]){
   const bars=[0,1,2,3].map((v,i)=>{const close=100+v*direction;return {timestampMs:now-(4-i)*120000,open:close-direction*.8,close,high:Math.max(close,close-direction*.8)+.1,low:Math.min(close,close-direction*.8)-.1,volume:100};});
   const result=new IntradayDiagnostics().update({...sample(),net:.6*direction,flow:-.7*direction,bars});
   expect(result.exhaustion).toContain('PARCIALES');expect(result.exhaustion).not.toContain('POSIBLE AGOTAMIENTO');
  }
 });
 it('clears a disappearing level and distinguishes decreasing structural weight',()=>{
  const t=new IntradayDiagnostics(),x=sample();t.update(x);
  const lower={...x,at:now+1000,levels:[{...x.levels[0],score:50,at:now+1000}]};
  expect(t.update(lower).levels[0]).toContain('PERDIENDO PESO');
  expect(t.update({...x,at:now+2000,levels:[]}).levels).toEqual([]);
  expect(t.update({...lower,at:now+3000}).levels[0]).toContain('OTRO SNAPSHOT');
 });
 it('compares same level only on newer snapshots, never on screen refresh',()=>{
  const t=new IntradayDiagnostics(),x=sample();expect(t.update(x).levels[0]).toContain('OTRO SNAPSHOT');
  expect(t.update({...x,at:now+1,levels:[{...x.levels[0],score:90}]}).levels[0]).toContain('OTRO SNAPSHOT');
  expect(t.update({...x,at:now+2,levels:[{...x.levels[0],score:70,at:now+2}]}).levels[0]).toContain('GANANDO PESO');
  expect(t.update({...x,at:now+3,levels:[{...x.levels[0],key:'call:106:expiry',level:106,score:95,at:now+3}]}).levels[0]).toContain('OTRO SNAPSHOT');
 });
 it('resets on disconnect, symbol, clock reversal and long gaps',()=>{
  for(const patch of [{ready:false},{session:'SPY:today:1'},{at:now-1},{at:now+16000}]){
   const t=new IntradayDiagnostics();t.update(sample());const r=t.update({...sample(),...patch});
   expect(r.levels.join()).not.toContain('GANANDO PESO');expect(r.exhaustion).toBe('EVIDENCIA INSUFICIENTE');
  }
 });
 it('requires same target to report approach and no target in equilibrium',()=>{
  const t=new IntradayDiagnostics();t.update(sample());
  expect(t.update({...sample(),spot:101,at:now+1000}).approach).toContain('ACERCÁNDOSE');
  expect(t.update({...sample(),spot:101,target:110,at:now+2000}).approach).toContain('ESPERANDO DESPLAZAMIENTO');
  expect(t.update({...sample(),net:0,at:now+3000}).distance).toBeNull();
 });
 it('needs price and flow confluence, not a wick alone',()=>{
  const bars=[100,101,102,102.1].map((close,i)=>({timestampMs:now-(4-i)*120000,open:close-.2,close,high:close+.3,low:close-.3,volume:100}));
  const t=new IntradayDiagnostics();const x={...sample(),bars};
  expect(t.update(x).exhaustion).toContain('PARCIALES');
  expect(t.update({...x,at:now+10000,flow:.4}).exhaustion).toContain('POSIBLE AGOTAMIENTO');
  expect(t.update({...x,at:now+11000,flow:null}).exhaustion).toBe('EVIDENCIA INSUFICIENTE');
 });
 it('rejects gaps in candles and missing flow from exhaustion',()=>{
  const x=sample();x.flow=-.8;x.bars=[0,1,3,4].map(i=>({timestampMs:now-(5-i)*120000,open:102,close:101,high:106,low:100,volume:1}));
  expect(new IntradayDiagnostics().update(x).exhaustion).toBe('EVIDENCIA INSUFICIENTE');
 });
});
