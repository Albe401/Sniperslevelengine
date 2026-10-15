import {expect,it} from 'vitest';
import {analystTargetsPanel,validTargets} from './analyst-targets';
const targets={symbol:'TEST',low:3.8,average:4.5,high:6,date:'2026-10-02',source:'FMP'};
it('computes signed distance from current price, not from the average target',()=>{
 const html=analystTargetsPanel('TEST',3.6,targets);
 expect(html).toContain('$3.80');expect(html).toContain('+5.6%');expect(html).toContain('+25.0%');
 expect(html).toContain('fecha original no informada');expect(html).not.toContain('Editar objetivos');
 expect(analystTargetsPanel('TEST',4,targets)).toContain('-5.0%');
});
it('withholds distances without a current reference or matching valid targets',()=>{
 expect(analystTargetsPanel('TEST',null,targets)).not.toContain('+5.6%');
 expect(analystTargetsPanel('OTHER',3.6,targets)).not.toContain('$3.80');
 for(const invalid of [{...targets,low:7},{...targets,average:NaN},{...targets,high:0}])expect(validTargets(invalid,'TEST')).toBe(false);
});
