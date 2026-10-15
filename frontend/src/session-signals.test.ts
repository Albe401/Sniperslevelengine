import {expect,it} from 'vitest';
import {sessionSignals,recentMovement} from './session-signals';
import type {ChartHistory} from './candle-series';
import type {LiveState} from './live';
const now=Date.parse('2026-09-25T14:00:00Z');
const history:ChartHistory={sessions:[{date:'2026-09-25',openMs:now-1800000,closeMs:now+21600000}],minutes:[504,503,502,501].map((close,i)=>({timestampMs:now-240000+i*60000,open:close,high:close+1,low:close-1,close,volume:100}))};
it('identifica descenso entre cierres sin convertir bajo VWAP en caída',()=>{
 expect(recentMovement(history,now)).toContain('Caída reciente · -2.00');
 const rising={...history,minutes:history.minutes.map((b,i)=>({...b,open:500+i,close:500+i,high:501+i,low:499+i}))};
 expect(recentMovement(rising,now)).toContain('Subida reciente · +2.00');
});
it('no une huecos ni utiliza velas incompletas o de otra fecha',()=>{
 expect(recentMovement({...history,minutes:history.minutes.filter((_,i)=>i!==2)},now)).toBe('Movimiento 2m pendiente');
 expect(recentMovement(history,now+181000)).toBe('Movimiento 2m pendiente');
 expect(recentMovement(history,now+86400000)).toBe('Movimiento 2m pendiente');
 expect(recentMovement(history,now-60000)).toBe('Movimiento 2m pendiente');
});
const state={symbol:'QQQ',publishedMs:now,stocks:{phase:'streaming'},metrics:{session:'2026-09-25',phase:'Sesión RTH',asofMs:now,vwap:501},ticks:{QQQ:{symbol:'QQQ',price:502,tradeMs:now}},structuralPaths:[{symbol:'QQQ',horizon:'DAY',candidate:{available:true,side:'CALL',blocks:[]},trigger:{ready:true,bar_end_ms:now-60000}}]} as unknown as LiveState;
it('muestra VWAP y confirmación cerrada del día sin autorizar entradas',()=>{
 const html=sessionSignals(state,'QQQ',true,now);expect(html).toContain('501.00');expect(html).toContain('Sobre VWAP');expect(html).toContain('CALL confirmado');expect(html).toContain('admisión pendiente');
});
it('rechaza datos de otra sesión, otro símbolo, desconectados y caducados',()=>{
 for(const html of [sessionSignals(state,'SPY',true,now),sessionSignals(state,'QQQ',false,now),sessionSignals(state,'QQQ',true,now+20000),sessionSignals({...state,metrics:{...state.metrics,session:'2026-09-24'}},'QQQ',true,now)]){expect(html).not.toContain('501.00');expect(html).not.toContain('CALL confirmado');}
});
it('no usa confirmaciones semanales, futuras o con evidencia inválida',()=>{
 const row=state.structuralPaths[0];
 for(const changed of [{...row,horizon:'WEEK'},{...row,trigger:{...row.trigger,bar_end_ms:now+1}},{...row,candidate:{...row.candidate,blocks:['UNVERIFIED_INPUT']}}])expect(sessionSignals({...state,structuralPaths:[changed]},'QQQ',true,now)).not.toContain('CALL confirmado');
});
