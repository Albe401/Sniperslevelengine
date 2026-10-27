import {expect,it} from 'vitest';
import {routePercentLabel,visibleRouteReading,type ProbabilityReading} from './route-probability-reading';
const empty:ProbabilityReading={call:null,put:null,state:'Sin probabilidad validada',detail:''};
it('labels raw confluence as an index, keeping probability unavailable for both directions',()=>{
 const score={call:75,put:25},reading=visibleRouteReading(empty,score);
 expect(reading.call).toBeNull();expect(reading.put).toBeNull();expect(reading.provisional).toEqual(score);
 for(const [side,value] of [[1,75],[-1,25]]){
  const label=routePercentLabel(reading,side);expect(label).toContain(`Confluencia ${value}/100`);expect(label).toContain('no es probabilidad');expect(label).not.toContain('%');
 }
 expect(empty).not.toHaveProperty('provisional');expect(score).toEqual({call:75,put:25});
});
it('preserves an actual model estimate and does not overwrite it with an uncalibrated index',()=>{
 const reading={...empty,call:63};expect(visibleRouteReading(reading,{call:90,put:10})).toBe(reading);
 expect(routePercentLabel(reading,1)).toBe('P(ref) 63% / 10 min');expect(routePercentLabel(reading,-1)).toBe('P(ref) sin probabilidad validada');
});
it('keeps unavailable and inactive scenarios explicit without manufacturing an index',()=>{
 expect(routePercentLabel(empty,1)).toBe('P(ref) sin probabilidad validada');expect(routePercentLabel(undefined,1)).toBe('P(ref) sin escenario activo');
 expect(visibleRouteReading(empty,{call:null,put:20})).toBe(empty);
 const baseline={...empty,kind:'baseline' as const};expect(visibleRouteReading(baseline,{call:80,put:20})).toBe(baseline);
});
