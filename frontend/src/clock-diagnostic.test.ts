import {it,expect} from 'vitest';
import {clockContent,type ClockReport} from './clock-diagnostic';
it('clock diagnostics never turn missing measurements into zero or sync certification',()=>{
 const r:ClockReport={checkedMs:0,source:'<source>',serviceStatusReadable:false,samples:0,medianOffsetMs:null,spreadMs:null,state:'UNVERIFIED',detail:'<failure>'};
 const html=clockContent(r,false,'',true);
 expect(html).toContain('SIN VERIFICAR');expect(html).toContain('Sin medida');expect(html).toContain('&lt;failure&gt;');expect(html).not.toContain('0.0 ms');
 expect(clockContent(undefined,false,'',false)).toContain('data-clock-check disabled');
 expect(clockContent({...r,samples:3,medianOffsetMs:2806.38,state:'OFFSET_DETECTED'},false,'',true)).toContain('2806.4 ms');
});
