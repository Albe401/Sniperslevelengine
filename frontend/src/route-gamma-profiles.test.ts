import {it,expect} from 'vitest';
import {routeProfileSvg} from './route-gamma-profiles';
import {gammaRouteChart} from './gamma-live';
it('offers the toggle only for the detailed chart',()=>{
 expect(gammaRouteChart(undefined,'SPY',false,Date.now(),true)).toContain('data-route-profiles');
 expect(gammaRouteChart(undefined,'SPY',false)).not.toContain('data-route-profiles');
});
it('projects each strike through the actual chart price coordinate',()=>{
 const rows=[{strike:100,net_gex:1000},{strike:101,net_gex:-500}];
 const html=routeProfileSvg(rows,100,560,p=>p===100?240:300);
 expect(html).toContain('y="240"');expect(html).toContain('y="300"');expect(html).toContain('#22dce8');expect(html).toContain('#ba51e6');
 expect(routeProfileSvg(rows,100,560,()=>null)).not.toContain('<rect');
});
it('converts signed GEX into gamma weighted by OI and multiplier',()=>{
 const html=routeProfileSvg([{strike:100,net_gex:1000}],100,560,()=>240,true);
 expect(html).toContain('Gamma ponderada firmada 10');expect(html).toContain('#26f46b');
 expect(routeProfileSvg([{strike:100,net_gex:1000}],0,560,()=>240,true)).toBe('');
});
