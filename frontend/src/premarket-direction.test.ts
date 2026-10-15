import {expect,it} from 'vitest';
import {premarketDirection} from './premarket-direction';
import {buildIntradayControl} from './intraday-control';
import {readDirectionScore,directionBadge} from './direction-scanner';
import {frameScore} from './investment-direction';
import type {LiveState} from './live';
function blocks(s:LiveState,buy=900000,sell=0){s.offExchange={analytics:{blocks:{symbol:'QQQ',date,premarket:{count:4,total:1000000,buy,sell,unknown:100000,complete:true,finalized:false,through_ms:now,top_levels:[]}}}} as unknown as LiveState['offExchange'];}
it('uses verified blocks without CVD and shares scanner result',()=>{const s=state();blocks(s);expect(premarketDirection(s,'QQQ',true,now)?.call).toBe(62);expect(frameScore('QQQ',[s],true,now).call).toBe(62);});
it.each(['foreign','previous','partial','finalized','stale','future','unknown','invalid'])('rejects unusable TRF %s',kind=>{const s=state();blocks(s);const r=s.offExchange.analytics.blocks!,p=r.premarket;if(kind==='foreign')r.symbol='SPY';if(kind==='previous')r.date='2026-10-02';if(kind==='partial')p.complete=false;if(kind==='finalized')p.finalized=true;if(kind==='stale')p.through_ms=now-15001;if(kind==='future')p.through_ms=now+1;if(kind==='unknown'){p.buy=100000;p.unknown=900000;}if(kind==='invalid')p.total=NaN;expect(premarketDirection(s,'QQQ',true,now)?.call).toBe(59);});
it('never adds aligned blocks to CVD and dampens conflict in both directions',()=>{for(const up of [true,false]){const s=state(up),buy=up?90:0,sell=up?0:90;s.cvd={symbol:'QQQ',date,at:now,available:true,premarket:{buy,sell,unknown:10,delta:buy-sell,sinceMs:now-600000,lastMs:now,gaps:0}} as unknown as LiveState['cvd'];const before=premarketDirection(s,'QQQ',true,now)!;blocks(s,buy*10000,sell*10000);expect(premarketDirection(s,'QQQ',true,now)?.call).toBe(before.call);blocks(s,sell*10000,buy*10000);const after=premarketDirection(s,'QQQ',true,now)!;expect(Math.abs(after.call!-50)).toBeLessThan(Math.abs(before.call!-50));expect(after.state).toContain('conflicto');}});
const now=Date.UTC(2026,9,5,12),date='2026-10-05';
function state(up=true):LiveState {
 const minutes=Array.from({length:6},(_,i)=>{const close=100+(up?1:-1)*i*.1;return {timestampMs:now-360000+i*60000,open:close,close,high:close+.01,low:close-.01,volume:100};});
 return {symbol:'QQQ',publishedMs:now,stocks:{phase:'streaming'},options:{phase:'closed'},ticks:{QQQ:{symbol:'QQQ',price:100,tradeMs:now}},metrics:{session:date,phase:'Esperando primera vela RTH completa'},flow:{intraday_windows:[]},minutes:Object.fromEntries(minutes.map(b=>[b.timestampMs,b])),chartHistory:{minutes:[],sessions:[{date,openMs:Date.UTC(2026,9,5,13,30),closeMs:Date.UTC(2026,9,5,20)}]}} as unknown as LiveState;
}
it('publishes bounded price-led premarket with options closed and no OI or CVD',()=>{
 const a=premarketDirection(state(),'QQQ',true,now)!;
 const b=premarketDirection(state(false),'QQQ',true,now)!;
 expect(a.call).toBe(59);expect(a.put).toBe(41);expect(b.call).toBe(41);expect(b.put).toBe(59);
 expect(a.state).toContain('preliminar por precio');expect(a.components).toHaveLength(1);
});
it('Quick and scanner use the same own-symbol premarket percentages',()=>{
 const s=state(),d=readDirectionScore(s,buildIntradayControl(s,'QQQ',true,now),true,now);
 expect(frameScore('QQQ',[s],true,now).call).toBe(d.call);
 expect(d.call).toBe(59);
 expect(directionBadge(d,undefined,{call:null,put:null,state:'Pendiente',detail:''})).toContain('CALL <strong>59%');
 expect(frameScore('SPY',[s],true,now).call).toBeNull();
});
it('uses verified CVD only and does not promote unknown volume to neutral evidence',()=>{
 const s=state();s.cvd={symbol:'QQQ',date,at:now,available:true,premarket:{buy:90,sell:0,unknown:10,delta:90,sinceMs:now-600000,lastMs:now,gaps:0}} as unknown as LiveState['cvd'];
 expect(premarketDirection(s,'QQQ',true,now)?.call).toBe(64);
 s.cvd!.premarket.unknown=1000;
 expect(premarketDirection(s,'QQQ',true,now)?.call).toBe(59);
});
it.each(['inactive','foreign','stale','future','tick','gap','duplicate','ohlc','holiday','RTH','old-session','bad-open'])('rejects invalid premarket input: %s',kind=>{
 const s=state();if(kind==='foreign')s.symbol='SPY';if(kind==='stale')s.publishedMs=now-3001;if(kind==='future')s.publishedMs=now+1;if(kind==='tick')s.ticks.QQQ.tradeMs=now-15001;
 const keys=Object.keys(s.minutes);if(kind==='gap')delete s.minutes[keys[0]];if(kind==='duplicate')s.minutes.extra={...s.minutes[keys[0]]};if(kind==='ohlc')s.minutes[keys[0]].high=1;
 if(kind==='holiday')s.metrics.phase='Sesión cerrada';if(kind==='RTH')s.chartHistory!.sessions[0].openMs=now;if(kind==='old-session')s.metrics.session='2026-10-02';if(kind==='bad-open')s.chartHistory!.sessions[0].openMs=NaN;
 expect(premarketDirection(s,'QQQ',kind!=='inactive',now)).toBeNull();
});

it('keeps five-asset premarket processing bounded',()=>{const frames=['QQQ','SPY','AAPL','NVDA','TSLA'].map(symbol=>{const s=state();blocks(s);s.symbol=symbol;s.ticks={[symbol]:{...s.ticks.QQQ,symbol}};s.offExchange.analytics.blocks!.symbol=symbol;return s;});const started=performance.now();for(let i=0;i<300;i++)for(const s of frames)premarketDirection(s,s.symbol,true,now);const elapsed=(performance.now()-started)/300;console.log(`Premarket five assets: ${elapsed.toFixed(3)} ms/cycle`);expect(elapsed).toBeLessThan(50);});
