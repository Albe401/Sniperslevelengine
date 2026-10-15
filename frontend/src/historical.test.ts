import {it,expect} from 'vitest';
import {historicalContent,type HistoricalReport,type HistoricalRead} from './historical';
it('keeps partial historical ranking distinct and shows coverage without entry claims',()=>{
  const row:HistoricalRead={symbol:'QQQ',date:'2026-09-23',status:'HISTORICO OBSERVADO',minuteCount:390,expectedMinutes:390,coverage:1,openFirstMinute:500,closeLastMinute:501,sessionChangePct:0.2,sessionHigh:502,sessionLow:499,shareVolume:100000,notionalApprox:50000000,rvol:null,rvolStatus:'SIN CINCO SESIONES COMPARABLES'};
  const data:HistoricalReport={date:row.date,phase:'partial',total:20,processed:20,rows:[{symbol:'QQQ',summary:row,error:null},{symbol:'SPY',summary:null,error:'<script>'}],top:[row],error:null,savedPath:null};
  const html=historicalContent(data,false,'',row.date,true,true);expect(html).toContain('RESULTADO PARCIAL');expect(html).toContain('390 / 390');expect(html).toContain('&lt;script&gt;');expect(html).not.toContain('<script>');
  data.phase='complete';expect(historicalContent(data,false,'',row.date,true,true)).toContain('UNIVERSO CONSULTADO');
  expect(historicalContent(undefined,false,'',row.date,false,true)).toContain('data-history-action="start" disabled');
  expect(historicalContent(data,true,'',row.date,true,true)).toContain('data-history-action="stop" >Cancelar');
});
