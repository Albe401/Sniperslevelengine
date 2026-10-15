import {expect,it} from 'vitest';
import {routeChartScenario} from './route-chart-scenario';
import type {RouteResult} from './route-engine';
const r:RouteResult={state:'sustained',direction:1,level:100,target:105,stop:99,at:1000,key:'q',reason:'test'};
it('draws only a conditioned destination and invalidation from the existing route',()=>{expect(routeChartScenario(r,101,1001,true)).toEqual({target:105,stop:99,direction:1});expect(routeChartScenario({...r,direction:-1,target:95,stop:103},101,1001,true)?.direction).toBe(-1);});
it.each(['waiting','range','recovery','test','invalidated','reached'] as const)('cannot project the %s stage',state=>expect(routeChartScenario({...r,state},101,1001,true)).toBeUndefined());
it('rejects stale, future, foreign/historical, passed-target and breached-stop scenarios',()=>{for(const value of [routeChartScenario(r,101,91001,true),routeChartScenario(r,101,999,true),routeChartScenario(r,101,1001,false),routeChartScenario(r,105,1001,true),routeChartScenario(r,98,1001,true),routeChartScenario({...r,target:NaN},101,1001,true)])expect(value).toBeUndefined();});
it.each([NaN,Infinity,-Infinity])('rejects nonfinite route and rendering clocks (%s)',clock=>{expect(routeChartScenario({...r,at:clock},101,1001,true)).toBeUndefined();expect(routeChartScenario(r,101,clock,true)).toBeUndefined();});
