import { expect,it } from 'vitest';
import { structuralReadingPanel } from './reading';
it('does not retain structural alignment through disconnection, pending query or expiry',()=>{
  const now=Date.parse('2026-09-24T10:30:00-04:00');
  const s={phase:'ready',observedMs:now,structuralAlignment:{status:'PARTIAL ALIGNMENT',side:'CALL',callCount:2,putCount:0,neutralCount:0,unavailableCount:1},horizons:[{mode:'DAY',label:'<day>',structure:{reading:{available:true,side:'CALL',reading:'ALCISTA CALL',strength:'STRONG',reason:'<reason>',bias:.8,callScore:90,putScore:10}}}]};
  const html=structuralReadingPanel(s,true,now);
  expect(html).toContain('Alineación parcial');expect(html).toContain('ALCISTA CALL');
  expect(html).toContain('&lt;day&gt;');expect(html).toContain('&lt;reason&gt;');
  expect(html).toContain('90.00 / 10.00');
  for(const phase of ['loading','failed']) expect(structuralReadingPanel({...s,phase},true,now)).not.toContain('ALCISTA CALL');
  expect(structuralReadingPanel(s,false,now)).not.toContain('Alineación parcial');
  expect(structuralReadingPanel(s,true,now+900_001)).not.toContain('ALCISTA CALL');
  const incomplete={...s,horizons:[{...s.horizons[0],structure:{reading:{...s.horizons[0].structure.reading,available:false}}}]};
  expect(structuralReadingPanel(incomplete,true,now)).not.toContain('90.00 / 10.00');
});
