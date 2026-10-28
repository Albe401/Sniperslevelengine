import {it,expect}from'vitest';import {CapturedChartPaint}from'./captured-chart-paint';
it('retains the live cadence and limits stationary archived work to one expiry/status pass per second',()=>{
 const gate=new CapturedChartPaint();for(let n=0;n<10;n++)expect(gate.shouldPaint(false,true,[1],100+n*100)).toBe(true);
 expect(gate.shouldPaint(true,true,[1],1000)).toBe(true);for(let n=1;n<10;n++)expect(gate.shouldPaint(true,true,[1],1000+n*100)).toBe(false);expect(gate.shouldPaint(true,true,[1],2000)).toBe(true);
});
it('repaints pan, price scale and pane changes immediately at the existing interaction cadence',()=>{
 const g=new CapturedChartPaint();expect(g.shouldPaint(true,true,[100,200],1000)).toBe(true);expect(g.shouldPaint(true,true,[101,200],1100)).toBe(true);expect(g.shouldPaint(true,true,[101,201],1200)).toBe(true);
});
it('does no offscreen work and resumes immediately when returning to the chart',()=>{
 const g=new CapturedChartPaint();expect(g.shouldPaint(true,true,[1],1000)).toBe(true);expect(g.shouldPaint(true,false,[1],1100)).toBe(false);expect(g.shouldPaint(true,true,[1],1200)).toBe(true);
});
it('rebind and a corrected wall clock cannot strand the previous rendering',()=>{
 const g=new CapturedChartPaint();g.shouldPaint(true,true,[1],10000);expect(g.shouldPaint(true,true,[1],9000)).toBe(true);g.invalidate();expect(g.shouldPaint(true,true,[1],9001)).toBe(true);
});
