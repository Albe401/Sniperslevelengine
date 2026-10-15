import {expect,it} from 'vitest';
import {adjustedReadingPanel} from './reading-adjusted';
it('labels applied evidence and masks invalid or stale adjusted readings',()=>{
 const r={reading:{available:true,side:'CALL',reading:'<CALL>',strength:'HIGH',reason:'',bias:0.2,callScore:60,putScore:40,confidence:63},flowApplied:true,trfApplied:true,trfConfidencePoints:3,trfScorePoints:1.5,reason:'<razón>'};
 const render=(active=true,published=1000,now=1000)=>adjustedReadingPanel({'<DAY>':r},null,active,published,now);
 expect(render()).toContain('&lt;CALL&gt;');expect(render()).toContain('TRF 3.00 pts');expect(render()).toContain('&lt;razón&gt;');
 expect(render(false)).not.toContain('TRF 3.00 pts');expect(render(true,1000,4001)).not.toContain('TRF 3.00 pts');
 r.reading.available=false;expect(render()).not.toContain('TRF 3.00 pts');
});
