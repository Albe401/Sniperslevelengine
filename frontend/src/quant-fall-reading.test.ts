import {expect,it} from 'vitest';
import type {LiveState} from './live';
import {quantFallPanel,quantFallReading} from './quant-fall-reading';
const start=Date.UTC(2026,9,21,18,30),now=start+20*60000;
const bars=Array.from({length:20},(_,i)=>({timestampMs:start+i*60000,open:727-i*.05,high:727.2-i*.05,low:726.8-i*.05,close:727-i*.05,volume:i>=17?400:100}));
function fixture():LiveState{return {symbol:'QQQ',publishedMs:now,chartHistory:{sessions:[{date:'2026-10-21',openMs:start,closeMs:start+390*60000}],minutes:bars},options:{phase:'streaming',lastEventMs:now},flow:{asof_ms:now,intraday_windows:[{minutes:5,event_count:10,contracts:100,premium:10000,premium_complete:true,bullish_pct:30,bearish_pct:70,direction:'BEARISH FLOW',buckets:{'CALL BUY':{contracts:10,premium:1000,trades:1},'CALL SELL':{contracts:20,premium:2000,trades:2},'PUT BUY':{contracts:50,premium:5000,trades:5},'PUT SELL':{contracts:0,premium:0,trades:0},UNKNOWN:{contracts:20,premium:2000,trades:2}}}]},optionSnapshotStatus:{phase:'ready',horizons:[{mode:'DAY',observedMs:now-60000,exposure:{gexComplete:true,netGex:-123456}}]}} as unknown as LiveState;}
it('measures closed three-minute decline, local relative volume, covered flow, and the own GEX regime',()=>{
 const r=quantFallReading(fixture(),'QQQ',true,now);
 expect(r.priceMovePct).toBeCloseTo(100*(bars[19].close/bars[16].close-1));expect(r.volumeRatio).toBe(4);expect(r.bearishFlowPct).toBe(70);expect(r.classifiedPct).toBe(80);expect(r.gexRegime).toBe('NEGATIVE');expect(r.criteria).toBe(3);
 const html=quantFallPanel(fixture(),'QQQ',true,now);expect(html).toContain('GEX NEGATIVO');expect(html).toContain('3/3 CRITERIOS OBSERVADOS');expect(html).not.toContain('MAPA DE ACEPTACIÓN');
});
it('does not use future bars, stale frames, foreign symbols, or incomplete GEX',()=>{
 const s=fixture();s.chartHistory!.minutes.push({...bars[19],timestampMs:now,close:700,volume:99999});expect(quantFallReading(s,'QQQ',true,now).volumeRatio).toBe(4);
 s.optionSnapshotStatus.horizons[0].exposure.gexComplete=false;expect(quantFallReading(s,'QQQ',true,now).gexRegime).toBe('PENDING');
 expect(quantFallReading(s,'SPY',true,now).criteria).toBeNull();expect(quantFallReading(s,'QQQ',true,now+16000).criteria).toBeNull();
});
it('keeps the archived reading at capture time without inventing missing market data',()=>{
 const s=fixture();
 expect(quantFallReading(s,'QQQ',true,now).criteria).toBe(3);
 expect(quantFallReading(s,'QQQ',true,now+20*60000).criteria).toBeNull();
 s.flow=undefined as unknown as LiveState['flow'];
 expect(quantFallReading(s,'QQQ',true,now).bearishFlowPct).toBeNull();
 expect(quantFallReading(s,'QQQ',true,now).criteria).toBeNull();
});
