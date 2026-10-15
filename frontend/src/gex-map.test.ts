import {it,expect} from 'vitest';
import {gexMap} from './gex-map';
import type {Horizon} from './live';
const now=Date.parse('2026-09-24T15:00:00Z');
const h={label:'Hoy',observedMs:now,expirations:['2026-09-24'],exposure:{gexComplete:true,spot:100,netGex:1000,zeroGamma:99,gammaRegime:'POSITIVE',strikes:[{strike:99,netGex:-500},{strike:101,netGex:1500},{strike:200,netGex:2000}]},structure:{complete:true,callWall:{strike:102},putWall:{strike:98}}} as unknown as Horizon;
const candles=[{timestampMs:now-120000,open:100,high:101,low:99,close:100.5,volume:10}];
it('renders signed exposure and walls with a shared price axis',()=>{
 const html=gexMap('QQQ',h,true,candles,2,now);
 expect(html).toContain('#28d8d0');expect(html).toContain('#d96bdf');
 expect(html).toContain('Strike 99.00');expect(html).toContain('Strike 101.00');expect(html).not.toContain('Strike 200.00');
 expect(html).toContain('CALL WALL 102.00');expect(html).toContain('ZERO GAMMA 99.00');
 expect(html).not.toContain('NaN');expect(html).not.toContain('Infinity');
 expect(html).toContain('class="market-chart"');expect(html).toContain('class="candle-volume"');
 expect(html).toContain('class="last-price"');expect(html).toContain('data-times=');
});
it('keeps prepared bars separate across missing time buckets and hides overlays on old sessions',()=>{
 const rows=[{...candles[0],timestampMs:now-600000,sessionDate:'2026-09-24',sessionVwap:100.1},{...candles[0],timestampMs:now-120000,sessionDate:'2026-09-24',sessionVwap:100.2}];
 const html=gexMap('QQQ',h,true,rows,2,now,{prepared:true});
 expect(html).toContain(`data-times="${now-600000},${now-120000}"`);
 expect(html.match(/class="candle-volume"/g)).toHaveLength(2);
 expect(html).not.toContain('<polyline');
});
it('hides stale and incomplete exposure while retaining today candles',()=>{
 for(const [snapshot,fresh] of [[h,false],[{...h,exposure:{...h.exposure,gexComplete:false}},true]] as const){
 const html=gexMap('QQQ',snapshot,fresh,candles,2,now);
 expect(html).not.toContain('class="gex-strike"');expect(html).not.toContain('CALL WALL 102.00');expect(html).toContain('O 100.00');
 }
});
it('does not show previous-session or future candles as current intraday data',()=>{
 const html=gexMap('QQQ',undefined,false,[{...candles[0],timestampMs:now-86400000},{...candles[0],timestampMs:now+60000}],2,now);
 expect(html).toContain('Esperando velas');expect(html).not.toContain('<svg');
});
it('historical session candles never receive current GEX overlays and preserve VWAP',()=>{
 const old=now-86400000;
 const prepared=[{...candles[0],timestampMs:old-120000,sessionDate:'2026-09-23',sessionVwap:100.2,minutesReceived:2,complete:true,provisional:false},{...candles[0],timestampMs:old,sessionDate:'2026-09-23',sessionVwap:100.3,minutesReceived:2,complete:true,provisional:false}];
 const html=gexMap('QQQ',h,true,prepared,2,now,{prepared:true,visible:48});
 expect(html).toContain('VELAS HISTÓRICAS');expect(html).not.toContain('class="gex-strike"');expect(html).not.toContain('CALL WALL 102.00');expect(html).toContain('<polyline');expect(html).toContain('VWAP 100.20');
});
it('shows received partial gamma without confirming a regime or walls',()=>{
 const partial={...h,exposure:{...h.exposure,gexComplete:false,gammaUsed:8,eligible:10}};
 const html=gexMap('QQQ',partial,true,candles,2,now);
 expect(html).toContain('GAMMA POR STRIKE');expect(html).toContain('8 / 10');
 expect(html).toContain('class="gex-strike"');expect(html).toContain('COBERTURA PARCIAL');
 expect(html).not.toContain('CALL WALL 102.00');expect(html).not.toContain('<b>POSITIVE</b>');
});
it('keeps the dated snapshot separate from historical candles and labels stale data',()=>{
 const rows=[{...candles[0],sessionDate:'2026-09-23',timestampMs:now-86400000}];
 const html=gexMap('QQQ',h,false,rows,2,now,{prepared:true});
 expect(html).toContain('GAMMA POR STRIKE');expect(html).toContain('ÚLTIMA CONSULTA · NO VIGENTE');
 expect(html).toContain('no representa el gamma histórico');expect(html).not.toContain('class="gex-strike"');
});
it('explains absent gamma without inventing a profile',()=>{
 const html=gexMap('QQQ',undefined,false,candles,2,now);
 expect(html).toContain('Gamma no disponible');expect(html).not.toContain('class="gex-profile-row"');
});
