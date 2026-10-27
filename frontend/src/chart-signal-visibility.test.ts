import {expect,it} from 'vitest';
import fs from 'node:fs';
import {CHART_SIGNAL_VISIBILITY_POLICY} from './chart-signal-visibility';
const source=(file:string)=>fs.readFileSync(new URL('./'+file,import.meta.url),'utf8');

it('keeps legacy CALL/PUT overlays off in both quick and detailed chart modes',()=>{
 for(const mode of ['quick','detailed']){
  expect({mode,visible:CHART_SIGNAL_VISIBILITY_POLICY.legacyCallPutOverlays,engine:CHART_SIGNAL_VISIBILITY_POLICY.engineObservationEnabled}).toEqual({mode,visible:false,engine:true});
 }
 expect(CHART_SIGNAL_VISIBILITY_POLICY.executionEnabled).toBe(false);expect(Object.isFrozen(CHART_SIGNAL_VISIBILITY_POLICY)).toBe(true);
 const gamma=source('gamma-live.ts');
 expect(gamma).toContain("import {CHART_SIGNAL_VISIBILITY_POLICY} from './chart-signal-visibility'");
 expect(gamma).toContain('const chartMarks=CHART_SIGNAL_VISIBILITY_POLICY.legacyCallPutOverlays?observed:[]');
 expect(gamma).toContain('if(enabled&&CHART_SIGNAL_VISIBILITY_POLICY.legacyCallPutOverlays&&!markers)');
 const draw=gamma.slice(gamma.indexOf('function drawRoute(){'),gamma.indexOf('function cleanup(){'));
 const guard=draw.indexOf('if(!CHART_SIGNAL_VISIBILITY_POLICY.legacyCallPutOverlays)');
 expect(guard).toBeGreaterThanOrEqual(0);expect(guard).toBeLessThan(draw.indexOf("document.createElementNS"));
 for(const marker of ["document.getElementById('route-intraday-status')",'cached.emaContext.label','cached.evaluation.label','Lectura anticipada:',"document.getElementById('route-4h-details')"]){expect(draw.indexOf(marker)).toBeGreaterThanOrEqual(0);expect(draw.indexOf(marker)).toBeLessThan(guard);}
 expect(draw.slice(guard,draw.indexOf('\n',guard))).toContain('markers?.detach()');
 expect(draw.slice(guard,draw.indexOf('\n',guard))).toContain('routeSvg?.remove()');
 expect(draw.slice(guard,draw.indexOf('\n',guard))).toContain('return;');
});

it('does not disable source readers, diagnostics, absorption, GEX or ordinary chart statistics',()=>{
 const route=source('route-live.ts'),gamma=source('gamma-live.ts');
 expect(route).toContain('sessionIntraday.read(');expect(route).toContain('engine.feed(');expect(route).toContain('buildChartSignalDiagnostics(');
 expect(route).not.toContain('CHART_SIGNAL_VISIBILITY_POLICY');
 for(const marker of ['routeStatsPrefix=','RSI','paintWebMassive','executionAbsorption','paintRouteProfiles'])expect(gamma).toContain(marker);
 expect(source('quick-reading-panels.ts')).toContain('gammaRouteChart(');
});
