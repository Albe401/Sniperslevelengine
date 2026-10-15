import {it,expect,vi} from 'vitest';
vi.mock('./price-now',()=>({readPriceNow:()=>({ready:true,spot:671})}));
vi.mock('./gamma-live',()=>({gammaRouteChart:()=>'<section id="real-route-chart"></section>'}));
import {quickChartLevels} from './quick-chart-levels';
import type {LiveState} from './live';
const now=Date.parse('2026-10-07T16:00:00Z');
function fixture(){return {symbol:'SPY',options:{phase:'idle'},flow:{},optionSnapshotStatus:{phase:'ready',observedMs:now,horizons:[{mode:'DAY',label:'Hoy',observedMs:now,expirations:['2026-10-07'],exposure:{gexComplete:true,scanComplete:true,zeroGamma:670},structure:{complete:true,routes:[{direction:'CALL',barrier:673,pullTarget:675,score:60},{direction:'PUT',barrier:667,pullTarget:665,score:50}],selected:{M1:671.5,M2:672.5},magnet:671,callWall:{strike:674},putWall:{strike:666}}}]}} as unknown as LiveState;}
it('uses the shared route chart and displays snapshot levels',()=>{
 const html=quickChartLevels(fixture(),'SPY',true,now);
 expect(html).toContain('real-route-chart');expect(html.indexOf('CALL BUY')).toBeGreaterThan(html.indexOf('Vacío inferior'));expect(html.indexOf('PUT BUY')).toBeGreaterThan(html.indexOf('CALL BUY'));expect(html.indexOf('LOW TARGET')).toBeGreaterThan(html.indexOf('PUT BUY'));expect(html).toContain('AVERAGE TARGET');expect(html).toContain('HIGH TARGET');expect(html).not.toContain('TARGET CALL</span>');expect(html).not.toContain('TARGET PUT</span>');expect(html).not.toContain('IMÁN DE LIQUIDEZ');expect(html).not.toContain('BARRERA DE LIQUIDEZ');expect(html).toContain('CALL PULL <b>675.00</b>');expect(html).toContain('PUT PULL <b>665.00</b>');expect(html).not.toContain('PRECIO ACTUAL');expect(html).not.toContain('day-map-spot');expect(html.indexOf('CALL WALL')).toBeLessThan(html.indexOf('PUT WALL'));for(const value of ['674.00','666.00','671.50'])expect(html).toContain(value);
});
it('hides levels when disconnected, expired, or for another symbol',()=>{
 for(const html of [quickChartLevels(fixture(),'SPY',false,now),quickChartLevels(fixture(),'QQQ',true,now),quickChartLevels(fixture(),'SPY',true,now+86400000)]){
 expect(html).not.toContain('674.00');expect(html).not.toContain('670.00');expect(html).toContain('Esperando nivel vigente');expect(html).toContain('LOW TARGET');
 }
});
it('never confirms zero gamma without a completed scan',()=>{
 const s=fixture();s.optionSnapshotStatus!.horizons[0].exposure.scanComplete=false;
 expect(quickChartLevels(s,'SPY',true,now)).not.toContain('670.00');
});






