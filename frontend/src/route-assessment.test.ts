import {expect,it} from 'vitest';
import {assessRoute} from './route-assessment';
import {routeChartStatus} from './route-chart-scenario';
import type {MarketEvidence} from './market-evidence';
import type {RouteResult} from './route-engine';
const now=100000;
const route=(direction=1):RouteResult=>({state:'sustained',direction,at:now,level:100,target:direction>0?105:95,stop:100,reason:'Precio observado',key:'r'});
const evidence=(direction=1)=>({at:now,price:{ready:true},direction,flow:{at:now,signal:direction*.5},cvd:{at:now,signal:direction*.3}} as MarketEvidence);
it.each([1,-1])('distinguishes aligned continuation from opposing pressure (%s)',d=>{
 const r=route(d),e=evidence(d);expect(assessRoute(r,e,now).quality).toBe('aligned');
 for(const field of ['flow','cvd'] as const){const x={...e,[field]:{...e[field],signal:-d*.2}};expect(assessRoute(r,x,now)).toMatchObject({quality:'opposed'});expect(routeChartStatus(r,true,x,now)).toContain('presión en contra');}
 expect(assessRoute(r,{...e,direction:-d},now).quality).toBe('opposed');
 expect(r).toEqual(route(d));
});
it.each([null,0,NaN])('missing or neutral pressure never appears aligned (%s)',signal=>{const e=evidence();e.cvd.signal=signal;expect(assessRoute(route(),e,now).quality).toBe('partial');});
it('withdraws stale/future price readings and ignores stale pressure',()=>{
 const r=route(),e=evidence();expect(assessRoute(r,{...e,at:now-3001},now).quality).toBe('pending');expect(assessRoute(r,{...e,at:now+1},now).quality).toBe('pending');
 expect(assessRoute({...r,at:now-90001},e,now).quality).toBe('pending');expect(assessRoute(r,{...e,cvd:{...e.cvd,at:now-3001}},now).quality).toBe('partial');
});
it('a directional bias does not activate price; candidates and tests retain their unmet condition',()=>{
 const e=evidence();expect(assessRoute({...route(),state:'range',direction:0},e,now).label).toContain('Sin activación');
 expect(assessRoute({...route(),state:'recovery'},e,now).label).toContain('falta prueba');expect(assessRoute({...route(),state:'test'},e,now).label).toContain('falta continuación');
 for(const state of ['invalidated','reached'] as const)expect(assessRoute({...route(),state,direction:0},e,now).label).toContain('nueva prueba');
});
