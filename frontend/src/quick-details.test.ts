import {expect,it} from 'vitest';
import {quickDetails} from './quick-details';
import type {LiveState} from './live';
const now=Date.parse('2026-09-25T15:00:00Z');
const state={symbol:'QQQ',publishedMs:now,stocks:{phase:'streaming'},options:{phase:'streaming',lastEventMs:now},ticks:{QQQ:{tradeMs:now}},optionSnapshotStatus:{phase:'ready',observedMs:now,horizons:[{mode:'DAY',observedMs:now,expirations:['2026-09-25'],structure:{complete:true,quickPressure:60,selected:{M1:745},callWall:{strike:755,score:70},putWall:{strike:727,score:80,zoneVolume:12345,zoneOi:45678},magnet:744,mainMagnetScore:72,liquidityMagnetScore:64,routes:[{direction:'CALL',pullTarget:750,pathLiquidity:20,pathOpenness:80,target:756,extended:760,vacuumLow:751,vacuumHigh:754,vacuumScore:90}]}}]},offExchange:{analytics:{session:{date:'2026-09-25',print_count:0,top_levels:[]},history:[{date:'2026-09-24',print_count:100,live_count:0,top_levels:[{level:741.25,strength_score:50,strength:'MEDIUM'}]}]}}} as unknown as LiveState;
it('renders compact scenarios, targets, zones and historical TRF without confirming it',()=>{
 const html=quickDetails(state,'QQQ',true,now);
 for(const text of ['Escenario CALL','Escenario PUT','755.00','756.00','760.00','751.00 – 754.00','60.00 %','TRF · histórico','741.25','SIN CONFIRMACIÓN DARK POOL','12.345','Fuerza 72.00/100','Fuerza 64.00/100'])expect(html).toContain(text);
});
it('hides stale, disconnected and foreign-symbol data',()=>{
 for(const html of [quickDetails(state,'SPY',true,now),quickDetails(state,'QQQ',false,now),quickDetails(state,'QQQ',true,now+20000)]){
  expect(html).not.toContain('755.00');expect(html).not.toContain('741.25');expect(html).not.toContain('60.00 %');
 }
});
it('renders empty state without fabricated numbers',()=>{const html=quickDetails(undefined,'QQQ',false,now);expect(html).toContain('ESPERANDO DATOS');expect(html).not.toContain('NaN');});
it('does not present invalid magnet strength as a valid score',()=>{
 const invalid=structuredClone(state) as LiveState;
 const structure=invalid.optionSnapshotStatus!.horizons[0].structure;
 structure.mainMagnetScore=Number.NaN;
 structure.liquidityMagnetScore=101;
 const html=quickDetails(invalid,'QQQ',true,now);
 expect(html).toContain('745.00');
 expect(html).toContain('744.00');
 expect(html).toContain('Fuerza pendiente');
 expect(html).not.toContain('Fuerza NaN/100');
 expect(html).not.toContain('Fuerza 101.00/100');
});
