import {expect,it} from 'vitest';
import {readPriceNow} from './price-now';
import {buildIntradayControl,controlPanel} from './intraday-control';
import type {LiveState} from './live';
const now=Date.parse('2026-09-28T15:00:20Z');
function fixture(closes=[738,737,736,735,734,733],spot=732.8):LiveState {
 const start=Date.parse('2026-09-28T14:54:00Z');
 return {symbol:'QQQ',publishedMs:now,startedMs:now-600000,stocks:{phase:'streaming'},options:{phase:'denied'},ticks:{QQQ:{symbol:'QQQ',price:spot,tradeMs:now}},metrics:{session:'2026-09-28',phase:'Sesión RTH',asofMs:now,vwap:737},flow:{intraday_windows:[]},chartHistory:{sessions:[{date:'2026-09-28',openMs:Date.parse('2026-09-28T13:30:00Z'),closeMs:Date.parse('2026-09-28T20:00:00Z')}],minutes:closes.map((close,i)=>({timestampMs:start+i*60000,open:close+.2,high:close+.3,low:close-.1,close,volume:1000}))}} as unknown as LiveState;
}
it('describes falling prices even with 20% model coverage and no options',()=>{
 const s=fixture(),r=buildIntradayControl(s,'QQQ',true,now),p=readPriceNow(s,'QQQ',true,now);
 expect(r.coverage).toBe(20);expect(p.headline).toBe('PRECIO CAYENDO');expect(p.structure).toContain('MÁS BAJOS');expect(p.pace).toBe('CAÍDA SOSTENIDA');expect(p.vwap).toContain('BAJO');expect(p.levels).toContain('BAJO MÍNIMO');
 const html=controlPanel(r).split('<details')[0];expect(html).toContain('Control de dealers');expect(html).not.toContain('control-hero');expect(html).not.toContain('NIVELES VIVOS');expect(html).not.toContain('PRECIO CAYENDO');expect(html).not.toContain('VENTAJA SIN DETERMINAR');expect(controlPanel(r)).not.toContain('PRECIO CAYENDO');expect(html).not.toContain('SIN DIRECCIÓN CLARA');expect(html).not.toContain('MOMENTUM');
});
it('distinguishes a live rebound during a falling window',()=>{const p=readPriceNow(fixture(undefined,733.5),'QQQ',true,now);expect(p.headline).toBe('REBOTE DURANTE LA CAÍDA');expect(p.pace).toContain('GIRO NO CONFIRMADO');});
it('shows rise and pullback symmetrically',()=>{expect(readPriceNow(fixture([731,732,733,734,735,736],736.3),'QQQ',true,now).headline).toBe('PRECIO SUBIENDO');expect(readPriceNow(fixture([731,732,733,734,735,736],735.5),'QQQ',true,now).headline).toBe('RETROCESO DURANTE LA SUBIDA');});
it('distinguishes noise from directional displacement',()=>{expect(readPriceNow(fixture([733,733.02,733,733.01,733,733.02],733.01),'QQQ',true,now).headline).toContain('LATERAL');});
it('requires current own-symbol stock data regardless of options',()=>{for(const [symbol,active,t] of [['SPY',true,now],['QQQ',false,now],['QQQ',true,now+3001]] as const)expect(readPriceNow(fixture(),symbol,active,t).ready).toBe(false);for(const time of [now+1001,now-15001]){const s=fixture();s.ticks.QQQ.tradeMs=time;expect(readPriceNow(s,'QQQ',true,now).ready).toBe(false);}});
it('handles the observed 600 ms clock lead explicitly without accepting large future timestamps',()=>{const s=fixture();s.ticks.QQQ.tradeMs=now+600;const p=readPriceNow(s,'QQQ',true,now);expect(p.headline).toBe('PRECIO CAYENDO');expect(p.at).toContain('reloj proveedor');s.ticks.QQQ.tradeMs=now+1001;expect(readPriceNow(s,'QQQ',true,now).ready).toBe(false);});
it('does not bridge history gaps or present stale bars as current',()=>{const s=fixture();s.chartHistory!.minutes.splice(3,1);expect(readPriceNow(s,'QQQ',true,now).ready).toBe(false);const stale=fixture();stale.chartHistory!.minutes.forEach(b=>b.timestampMs-=600000);expect(readPriceNow(stale,'QQQ',true,now).headline).toContain('HISTORIAL PENDIENTE');});
it('does not use a prior session or stale VWAP',()=>{const s=fixture();s.chartHistory!.sessions[0].date='2026-09-25';expect(readPriceNow(s,'QQQ',true,now).ready).toBe(false);const other=fixture();other.metrics.asofMs=now-180001;expect(readPriceNow(other,'QQQ',true,now).vwap).toBe('VWAP pendiente');});
it('reacts to the latest trade without waiting for a new closed bar',()=>{const s=fixture();expect(readPriceNow(s,'QQQ',true,now).headline).toBe('PRECIO CAYENDO');s.ticks.QQQ.price=734;expect(readPriceNow(s,'QQQ',true,now).headline).toBe('REBOTE DURANTE LA CAÍDA');});
it('reports conflicting options without overriding the stock direction',()=>{const s=fixture();s.options.phase='streaming';s.options.lastEventMs=now;s.flow.intraday_windows=[{minutes:1,event_count:10,premium_complete:true,bullish_pct:90,bearish_pct:10} as never];const p=readPriceNow(s,'QQQ',true,now);expect(p.headline).toBe('PRECIO CAYENDO');expect(p.flow).toContain('contradicen');s.flow.intraday_windows[0].bullish_pct=NaN;expect(readPriceNow(s,'QQQ',true,now).flow).toContain('incompleta');});

it('shows real premarket price control from raw minutes while the chart history is RTH-only',()=>{const pre=Date.parse('2026-09-28T12:00:20Z'),s=fixture();s.publishedMs=pre;s.ticks.QQQ.tradeMs=pre;s.metrics.phase='Esperando primera vela RTH completa';s.chartHistory!.minutes.forEach((b,i)=>b.timestampMs=pre-20000-360000+i*60000);s.minutes=Object.fromEntries(s.chartHistory!.minutes.map(b=>[b.timestampMs,b]));s.chartHistory!.minutes=[];const p=readPriceNow(s,'QQQ',true,pre);expect(p.ready).toBe(true);expect(p.premarket).toBe(true);expect(p.headline).toBe('PRECIO CAYENDO');expect(p.flow).toContain('Premarket');const html=controlPanel(buildIntradayControl(s,'QQQ',true,pre),true);expect(html).toContain('ESPERANDO DATOS');expect(html).not.toContain('PRECIO EN ESTE MOMENTO');s.minutes={};expect(readPriceNow(s,'QQQ',true,pre).ready).toBe(false);});


it('does not close a raw minute after its source publication merely because the evaluator crossed the boundary',()=>{
 const boundary=now-20000,s=fixture();s.publishedMs=boundary-1;s.ticks.QQQ.tradeMs=boundary-1;s.metrics.asofMs=boundary-1;
 Object.assign(s.chartHistory!.minutes.at(-1)!,{open:733,high:1001,low:732.9,close:1000});
 const original=readPriceNow(s,'QQQ',true,s.publishedMs),delayed=readPriceNow(s,'QQQ',true,boundary+1);
 expect(original.ready).toBe(true);expect(delayed).toEqual(original);
 s.publishedMs=boundary+1;expect(readPriceNow(s,'QQQ',true,boundary+2)).not.toEqual(original);
});
