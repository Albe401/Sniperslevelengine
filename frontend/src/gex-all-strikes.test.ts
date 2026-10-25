import {expect,it} from 'vitest';
import real from './gex-strikes.real.json';
import {massiveExposurePanel} from './massive-exposure-panel';
import {readMassiveExposure} from './massive-exposure';
import {selectGexWindow,GEX_WINDOWS} from './gex-windows';
import type {LiveState} from './live';
const now=real.observedMs;
function frame():LiveState{return {symbol:'QQQ',publishedMs:now,contractChain:{observation:{observedMs:now,value:{symbol:'QQQ',days:183,gexWindows:real.report}}}} as unknown as LiveState;}
it('reproduces the reported 669/450 first-page cutoffs while proving higher strikes were received',()=>{for(const [mode,last]of [['WEEK_7D',669],['MONTH_6',450]] as const){const w=real.report.windows.find(w=>w.mode===mode)!;expect(w.rows[99].strike).toBe(last);expect(w.rows.some(r=>r.strike===695)).toBe(true);expect(w.rows.some(r=>r.strike===700)).toBe(true);expect(w.rows.at(-1)!.strike).toBeGreaterThan(700);}});
it.each(GEX_WINDOWS.map(([mode])=>mode))('renders every received strike exactly once in a continuous %s list',mode=>{const s=frame();selectGexWindow('QQQ',mode);try{const html=massiveExposurePanel(s,'QQQ',true,now,true),v=readMassiveExposure(s,'QQQ',true,now);const rows=[...html.matchAll(/data-massive-exposure-strike="([^"]+)" data-massive-gex="([^"]+)"/g)].map(m=>({strike:Number(m[1]),net:Number(m[2])}));expect(rows).toEqual(v.rows.map(r=>({strike:r.strike,net:r.netGex})));expect(new Set(rows.map(r=>r.strike)).size).toBe(rows.length);expect(html).not.toContain('data-massive-exposure-page=');if(v.rows.length){expect(html).toContain('Todos los strikes');for(const strike of [500,600,669,695,700,705,710])if(v.rows.some(r=>r.strike===strike))expect(rows.some(r=>r.strike===strike)).toBe(true);expect(rows.at(-1)!.strike).toBeGreaterThan(700);}}finally{selectGexWindow('QQQ','DAY');}});
