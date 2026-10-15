import { expect, it } from 'vitest';
import { dexChangePanel, dexUsable, type DexChange } from './dex-change';
const now=Date.parse('2026-09-24T10:30:00-04:00');
const change:DexChange={available:true,side:'CALL',reason:'<unsafe>',call:9,put:0,net:9,pctGross:0.5,previousNet:191,previousMs:now-60_000,observedMs:now};
it('hides a change when disconnected, pending or stale and escapes provider text',()=>{
  const ready={phase:'ready',dexChanges:{DAY:change}};
  expect(dexChangePanel(ready,true,now)).toContain('<td>CALL<');
  expect(dexChangePanel(ready,true,now)).toContain('&lt;unsafe&gt;');
  expect(dexChangePanel(ready,false,now)).not.toContain('<td>CALL<');
  expect(dexChangePanel({...ready,phase:'loading'},true,now)).not.toContain('<td>CALL<');
  expect(dexChangePanel(ready,true,now+900_001)).toContain('Referencia caducada');
});
it('rejects future timestamps and the next ET session even inside the age limit',()=>{
  expect(dexUsable(change,now-1)).toBe(false);
  const midnight=Date.parse('2026-09-25T00:00:00-04:00');
  expect(dexUsable({...change,observedMs:midnight-1},midnight)).toBe(false);
});
