import {expect,it} from 'vitest';
import {intradayReading} from './intraday-reading';
import type {LiveState} from './live';
const now=Date.parse('2026-09-24T15:00:00Z');
const fixture=()=>({symbol:'QQQ',publishedMs:now,
  stocks:{phase:'streaming'},options:{phase:'streaming',lastEventMs:now},
  ticks:{QQQ:{symbol:'QQQ',price:730.24,tradeMs:now}},
  metrics:{session:'2026-09-24',phase:'Sesión RTH',vwap:729.80,rvol:1.42,volume:123456,baselineSessions:5,asofMs:now-120000},
  flow:{windows:[{minutes:1,event_count:1,contracts:1234,premium:654321,premium_complete:true,direction:'BULLISH'}]},
  offExchange:{analytics:{session:{date:'2026-09-24',total_notional:2500000,print_count:29,complete:false,top_levels:[{level:731.11}]},live_fresh:true,last_live_ms:now}}
} as unknown as LiveState);
it('separa precio, VWAP y flujos observados sin convertirlos en permisos',()=>{
 const html=intradayReading(fixture(),'QQQ',true,now);
 expect(html).toContain('Precio sobre VWAP');expect(html).toContain('730.24');expect(html).toContain('729.80');
 expect(html).toContain('731.11');expect(html).toContain('Cobertura parcial');
 expect(html).toContain('no solo 0DTE');expect(html).toContain('no dinero neto entrando');
});
it('no filtra valores de otro activo, desconectado o de publicación caducada',()=>{
 for(const html of [intradayReading(fixture(),'SPY',true,now),intradayReading(fixture(),'QQQ',false,now),intradayReading(fixture(),'QQQ',true,now+3001),intradayReading(undefined,'QQQ',false,now)]) {
  expect(html).not.toContain('730.24');expect(html).not.toContain('729.80');expect(html).not.toContain('731.11');
 }
});
it('un quote reciente no rejuvenece el último trade',()=>{
 const s=fixture();s.ticks.QQQ.tradeMs=now-15001;
 expect(intradayReading(s,'QQQ',true,now)).not.toContain('730.24');
 s.ticks.QQQ.tradeMs=now+1;
 expect(intradayReading(s,'QQQ',true,now)).not.toContain('730.24');
});
it('rechaza métricas de otra fecha, antiguas y fuera de RTH aunque el feed esté vivo',()=>{
 for(const patch of [{session:'2026-09-23'},{asofMs:now-180001},{asofMs:now+1},{phase:'Cierre RTH'}]) {
  const s=fixture();Object.assign(s.metrics,patch);
  const html=intradayReading(s,'QQQ',true,now);
  expect(html).not.toContain('729.80');expect(html).not.toContain('Precio sobre VWAP');
 }
});
it('separa el acumulado TRF de hoy de los prints recientes y del histórico',()=>{
 const s=fixture();s.offExchange.analytics.last_live_ms=now-16000;
 expect(intradayReading(s,'QQQ',true,now)).toContain('ACUMULADO · SIN PRINT RECIENTE');
 s.offExchange.analytics.session.date='2026-09-23';
 expect(intradayReading(s,'QQQ',true,now)).not.toContain('731.11');
});
it('no publica prima incompleta ni sesgo de una ventana sin operaciones',()=>{
 const s=fixture();s.flow.windows[0].premium_complete=false;s.flow.windows[0].event_count=0;
 let html=intradayReading(s,'QQQ',true,now);
 expect(html).not.toContain('BULLISH');expect(html).not.toContain('654');
 s.options.lastEventMs=now-15001;html=intradayReading(s,'QQQ',true,now);
 expect(html).toContain('Sin flujo vigente');
});
