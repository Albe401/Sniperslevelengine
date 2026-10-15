import {it,expect} from 'vitest';
import {visibleRouteReading,routePercentLabel,type ProbabilityReading} from './route-probability-reading';
import {directionBadge,combineDirection} from './direction-scanner';
const pending:ProbabilityReading={call:null,put:null,state:'Pendiente',detail:'Sin resultados suficientes'};
it('keeps the current asset confluence visible when route probability has no scenario',()=>{
 const d=combineDirection(.5,.5,.5,60);
 const html=directionBadge(d,undefined,pending);
 expect(html).toContain(`CALL <strong>${d.call}%`);
 expect(html).toContain(`PUT <strong>${d.put}%`);
 expect(html).toContain('CONF. PROVISIONAL');
 expect(pending.provisional).toBeUndefined();
});
it('shows provisional percentages consistently without calling them probabilities',()=>{
 const reading=visibleRouteReading(pending,{call:70,put:30});
 const html=directionBadge(combineDirection(.5,.5,.5,60),undefined,reading);
 expect(html).toContain('CONF. PROVISIONAL');expect(html).toContain('CALL <strong>70%');expect(html).toContain('PUT <strong>30%');expect(html).toContain('Sin calibrar');
 expect(reading.call).toBeNull();expect(routePercentLabel(reading,1)).toBe('Conf. 70% · provisional');expect(routePercentLabel(reading,-1)).toBe('Conf. 30% · provisional');
});
it('retains the starting provisional score instead of replacing it with a later score',()=>{
 const first=visibleRouteReading(pending,{call:70,put:30});
 expect(visibleRouteReading(first,{call:55,put:45}).provisional).toEqual({call:70,put:30});
});
it('keeps incomplete readings empty and prioritizes calibrated probabilities',()=>{
 expect(visibleRouteReading(pending,{call:null,put:null}).provisional).toBeUndefined();
 const validated={...pending,call:68};expect(visibleRouteReading(validated,{call:70,put:30}).provisional).toBeUndefined();expect(routePercentLabel(validated,1)).toBe('P(ref) 68% / 10 min');expect(routePercentLabel(validated,-1)).toBe('P(ref) pendiente');
});
