import {it,expect} from 'vitest';
import {SessionIntradayRoute,sessionRouteLevels,intradayRoutePanel} from './session-intraday-route';
import type {LiveState} from './live';
import type {MarketEvidence} from './market-evidence';
import {readRouteLive,routeChartReading,routeJournal} from './route-live';
import {routeChartScenario} from './route-chart-scenario';
const start=Date.UTC(2026,9,8,13,30),date='2026-10-08';
const bar=(i:number,open=100.2,high=100.3,low=100.1,close=100.2,volume=100)=>({timestampMs:start+i*60000,open,high,low,close,volume,vwap:close});
const base=Array.from({length:8},(_,i)=>bar(i));base[2]=bar(2,100.2,100.3,100,100.2);base[4]=bar(4,100.2,102,100.1,100.2);
const sequence=[bar(8,100,100.5,99.8,100.4,200),bar(9,100.2,100.3,99.99,100.15,80),bar(10,100.15,100.7,100.1,100.6,150)];
function frame(bars:typeof base,sign=1){const now=bars.at(-1)!.timestampMs+60000,symbol='AMZN',regular={buy:sign===1?800:100,sell:sign===1?100:800,unknown:100,delta:700*sign,lastMs:now,sinceMs:start,gaps:0,coveragePct:90,changeOne:0,changeFive:700*sign};return {symbol,publishedMs:now,startedMs:start,stocks:{phase:'streaming'},options:{phase:'closed'},ticks:{[symbol]:{symbol,price:bars.at(-1)!.close,tradeMs:now}},metrics:{session:date,phase:'Sesión RTH'},flow:{intraday_windows:[]},chartHistory:{sessions:[{date,openMs:start,closeMs:start+390*60000}],minutes:bars},cvd:{symbol,date,at:now,available:true,regular,premarket:regular,rejected:0,recentFive:{buy:regular.buy,sell:regular.sell,unknown:100,delta:regular.delta,startMs:now-300000,endMs:now,complete:true}}} as unknown as LiveState;}
function market(s:LiveState){return {symbol:s.symbol,at:s.publishedMs,flow:{signal:null,at:null,coverage:null}} as unknown as MarketEvidence;}
function run(sign=1,alter?:(s:LiveState)=>void){const engine=new SessionIntradayRoute(),reflect=(b:typeof base[number])=>sign===1?{...b}:{...b,open:200-b.open,high:200-b.low,low:200-b.high,close:200-b.close,vwap:200-b.vwap};let bars=base.map(reflect),s=frame(bars,sign);engine.read(s,s.symbol,true,market(s),s.publishedMs);let result;for(const raw of sequence){bars=[...bars,reflect(raw)];s=frame(bars,sign);alter?.(s);result=engine.read(s,s.symbol,true,market(s),s.publishedMs);}return {engine,bars,s,result:result!};}
it.each([1,-1])('can confirm a fresh intraday %s route without 0DTE or a swing target',sign=>{const {result}=run(sign);expect(result.state).toBe('confirmed');expect(result.side).toBe(sign===1?'CALL':'PUT');expect(result.target).toBe(sign===1?102:98);expect(result.executionEnabled).toBe(false);expect(result.basis).toBe('precio + CVD');expect(result.reason).toContain('Opciones sin confirmación comparable');expect(result.signals).toHaveLength(1);});
it('warmup cannot manufacture a historical signal',()=>{const s=frame([...base,...sequence]),e=new SessionIntradayRoute(),r=e.read(s,s.symbol,true,market(s),s.publishedMs);expect(r.state).toBe('waiting');expect(r.signals).toEqual([]);});
it.each(['missing','opposed','partial','stale','corrupt','unknown'])('does not confirm with %s CVD',kind=>{const {result}=run(1,s=>{if(kind==='missing')s.cvd=undefined;else if(kind==='opposed')Object.assign(s.cvd!.recentFive!,{buy:100,sell:800,delta:-700});else if(kind==='partial')s.cvd!.recentFive!.complete=false;else if(kind==='stale')s.cvd!.at-=4000;else if(kind==='corrupt')s.cvd!.recentFive!.delta=999;else Object.assign(s.cvd!.recentFive!,{buy:100,sell:0,unknown:1000,delta:100});});expect(result.state).toBe('blocked');expect(result.signals).toEqual([]);});
it('duplicates cannot add confirmations or repeated markers',()=>{const {engine,s}=run();for(let i=0;i<20;i++)expect(engine.read(s,s.symbol,true,market(s),s.publishedMs).signals).toHaveLength(1);});
it('fresh contrary options veto confirmation while absence is explicit',()=>{const {engine,s}=run();const m=market(s);m.flow={signal:-.5,coverage:90,at:s.publishedMs,reason:'test'};const r=engine.read(s,s.symbol,true,m,s.publishedMs);expect(r.state).toBe('blocked');expect(r.reason).toContain('Flujo de opciones vigente en contra');});
it.each(['disconnect','stale','reconnect','foreign','close','correction'])('cannot retain confirmation after %s',kind=>{const {engine,s}=run();let now=s.publishedMs;if(kind==='stale')now+=4000;if(kind==='reconnect')s.startedMs+=1;if(kind==='foreign')s.symbol='SPY';if(kind==='close')now=s.chartHistory!.sessions[0].closeMs;if(kind==='correction')s.chartHistory!.minutes.at(-1)!.volume+=1;const r=engine.read(s,'AMZN',kind!=='disconnect',market(s),now);expect(r.state).toBe('waiting');expect(r.signals).toEqual([]);});
it('does not chase an almost reached reference or move the target',()=>{const {engine,s}=run();s.ticks.AMZN.price=101.9;const r=engine.read(s,s.symbol,true,market(s),s.publishedMs);expect(r.state).toBe('blocked');expect(r.target).toBe(102);expect(r.reason).toContain('1,8');});
it('session pivots are unavailable before the two confirming closes',()=>{const before=frame(base.slice(0,4)),after=frame(base);expect(sessionRouteLevels(before,before.publishedMs).some(l=>l.price===100)).toBe(false);expect(sessionRouteLevels(after,after.publishedMs).some(l=>l.price===100)).toBe(true);expect(sessionRouteLevels(after,after.publishedMs).every(l=>l.source==='session-price'&&l.expiration===date)).toBe(true);});
it('never imports next-expiry option walls into session references',()=>{const {engine,s}=run();s.optionSnapshotStatus={horizons:[{expirations:['2026-10-09'],structure:{callWall:{strike:500},putWall:{strike:1}}}]} as LiveState['optionSnapshotStatus'];expect(sessionRouteLevels(s,s.publishedMs).some(l=>l.price===500||l.price===1)).toBe(false);expect(engine.read(s,s.symbol,true,market(s),s.publishedMs).target).toBe(102);});
it.each([1,-1])('resolves %s only on a new closed candle, without electing the reverse',sign=>{const {engine,s,bars}=run(sign);const close=sign===1?102.1:97.9;const next=frame([...bars,bar(11,close,close+.1,close-.1,close)],sign);const r=engine.read(next,next.symbol,true,market(next),next.publishedMs);expect(r.state).toBe('reached');expect(r.target).toBeNull();expect(r.side).toBe('PENDIENTE');});
it('a lost anchor invalidates instead of keeping a live signal',()=>{const {engine,bars}=run();const s=frame([...bars,bar(11,100,100.1,99.7,99.8,200)]);const r=engine.read(s,s.symbol,true,market(s),s.publishedMs);expect(r.state).toBe('invalidated');expect(r.target).toBeNull();});
it('does not convert a contrary swing context into an intraday veto or target',()=>{const {engine,s}=run();s.swingReport={context:{side:'PUT'}} as LiveState['swingReport'];const r=engine.read(s,s.symbol,true,market(s),s.publishedMs);expect(r.state).toBe('confirmed');expect(r.side).toBe('CALL');expect(r.target).toBe(102);});
it('a gap requires a new sequence and does not mutate a former observed result',()=>{const {engine,result,bars}=run(),saved=JSON.stringify(result);const s=frame([...bars,bar(12,100.6,100.9,100.5,100.8)]);expect(engine.read(s,s.symbol,true,market(s),s.publishedMs).state).toBe('waiting');expect(JSON.stringify(result)).toBe(saved);});
it('respects the actual calendar close on shortened sessions',()=>{const {engine,s}=run();s.chartHistory!.sessions[0].closeMs=s.publishedMs;expect(engine.read(s,s.symbol,true,market(s),s.publishedMs).state).toBe('waiting');});
it('rejects incomplete input and nonfinite clocks safely',()=>{const e=new SessionIntradayRoute();for(const now of [start,NaN,Infinity])expect(e.read({symbol:'AMZN'} as LiveState,'AMZN',true,{symbol:'AMZN'} as MarketEvidence,now).state).toBe('waiting');});
it('exposes the actual basis, target and invalidation without a fabricated probability',()=>{const {result}=run();const html=intradayRoutePanel(result);expect(html).toContain('precio + CVD');expect(html).toContain('$102.00');expect(html).toContain('$99.97');expect(html).toContain('experimental');expect(html).not.toContain('%');expect(intradayRoutePanel({...result,reason:'<script>bad</script>'})).not.toContain('<script>');});
it('connects the actual session strategy to the chart and journal while 0DTE remains unavailable',()=>{
 let bars=base.slice(),s=frame(bars);readRouteLive(s,s.symbol,true,s.publishedMs);
 for(const b of sequence){bars=[...bars,b];s=frame(bars);readRouteLive(s,s.symbol,true,s.publishedMs);}
 const panel=readRouteLive(s,s.symbol,true,s.publishedMs),chart=routeChartReading(s.symbol,s.publishedMs)!;
 expect(panel.result.state).toBe('waiting');expect(panel.intraday.state).toBe('confirmed');expect(chart.intraday).toEqual(panel.intraday);
 expect(routeChartScenario(chart.intraday.result,s.ticks.AMZN.price!,s.publishedMs,true)?.target).toBe(102);
 expect(JSON.parse(routeJournal()).records.at(-1).intraday.state).toBe('confirmed');
 chart.intraday.result.target=999;chart.intraday.signals[0].text='changed';
 expect(routeChartReading(s.symbol,s.publishedMs)!.intraday.result.target).toBe(102);
 expect(routeChartReading(s.symbol,s.publishedMs)!.intraday.signals[0].text).toContain('precio + CVD');
 expect(routeChartReading(s.symbol,s.publishedMs+3001)).toBeUndefined();
});
it.each([1,-1])('identifies the structural destination for direction %s',sign=>{const {result}=run(sign);expect(result.targetStructure).toBe(sign===1?'Máximo estructural confirmado':'Mínimo estructural confirmado');expect(intradayRoutePanel(result)).toContain('Posible llegada');});
it('resets if a previously observed candle disappears',()=>{const {engine,s}=run();s.chartHistory!.minutes=s.chartHistory!.minutes.filter((_,i)=>i!==6);expect(engine.read(s,s.symbol,true,market(s),s.publishedMs).state).toBe('waiting');});
it('resets on a backwards clock instead of retaining a confirmation',()=>{const {engine,s}=run();expect(engine.read(s,s.symbol,true,market(s),s.publishedMs-1).state).not.toBe('confirmed');});
it('does not let consumers modify stored signal evidence',()=>{const {engine,s,result}=run();result.signals[0].text='tampered';expect(engine.read(s,s.symbol,true,market(s),s.publishedMs).signals[0].text).not.toBe('tampered');});
it('handles absent option evidence as unavailable instead of throwing',()=>{const {engine,s}=run();const r=engine.read(s,s.symbol,true,{symbol:s.symbol,at:s.publishedMs} as MarketEvidence,s.publishedMs);expect(r.state).toBe('confirmed');expect(r.reason).toContain('sin confirmación comparable');});
it.each(['call-wall','put-wall','barrier'])('does not ignore a strong %s before the destination',role=>{const {engine,s}=run();const m=market(s);m.levels=[{name:role,role:role as import('./route-engine').RouteLevel['role'],price:101,strength:85,observed:s.publishedMs,asof:s.publishedMs,expiration:date,provisional:false}];const r=engine.read(s,s.symbol,true,m,s.publishedMs);expect(r.state).toBe('blocked');expect(r.reason).toContain('antes de la llegada');expect(r.target).toBe(102);expect(r.levelReading).toContain('85.0/100');});
it.each([45,85])('uses the existing backing score %s without inventing a new one',strength=>{const {engine,s}=run();const m=market(s);m.levels=[{name:'PUT WALL',role:'put-wall',price:100,strength,change:0,observed:s.publishedMs,asof:s.publishedMs,expiration:date,provisional:false}];const r=engine.read(s,s.symbol,true,m,s.publishedMs);expect(r.state).toBe(strength<60?'blocked':'confirmed');expect(r.levelReading).toContain(`${strength.toFixed(1)}/100`);});
it.each(['expired','foreign','stale'])('ignores %s important levels',kind=>{const {engine,s}=run();const m=market(s);m.levels=[{name:'CALL WALL',role:'call-wall',price:101,strength:90,observed:s.publishedMs,asof:kind==='stale'?s.publishedMs-300001:s.publishedMs,expiration:kind==='expired'?'2026-10-09':date,provisional:false}];if(kind==='foreign')m.symbol='SPY';expect(engine.read(s,s.symbol,true,m,s.publishedMs).state).toBe('confirmed');});
it('adds ATR magnitude to the actual route without moving the structural destination',()=>{const {engine,s}=run();const atr={value:.5,percent:.5,lastRangeRatio:1,endMs:s.publishedMs,reason:'test'};const r=engine.read(s,s.symbol,true,market(s),s.publishedMs,atr);expect(r.state).toBe('confirmed');expect(r.target).toBe(102);expect(r.atr!.targetAtr).toBeCloseTo(2.8);expect(intradayRoutePanel(r)).toContain('hasta llegada');});
it('shows missing ATR explicitly without fabricating a measurement',()=>{const {result}=run();expect(result.atr!.value).toBeNull();expect(intradayRoutePanel(result)).toContain('ATR 5 min pendiente');});

it.each([45,85])('extra closes preserve the existing backing score %s policy and source sequence',strength=>{
 const engine=new SessionIntradayRoute();let bars=base.slice(),s=frame(bars);
 const evidence=()=>{const m=market(s);m.levels=[{name:'PUT WALL',role:'put-wall',price:100,strength,dynamicStrength:99,change:0,observed:start,asof:s.publishedMs,expiration:date,provisional:false}];return m;};
 engine.read(s,s.symbol,true,evidence(),s.publishedMs);
 const reads=[];
 for(const b of [...sequence,bar(11,100.6,100.75,100.5,100.65,150),bar(12,100.65,100.76,100.55,100.66,150)]){
  bars=[...bars,b];s=frame(bars);const m=evidence(),saved=JSON.stringify(m.levels);
  reads.push(engine.read(s,s.symbol,true,m,s.publishedMs));expect(JSON.stringify(m.levels)).toBe(saved);
 }
 const final=reads.at(-1)!;
 expect(final.result.state).toBe('sustained');
 expect(final.levelReading).toContain(`${strength.toFixed(1)}/100`);expect(final.target).toBe(102);expect(final.executionEnabled).toBe(false);
 if(strength<60){
  expect(reads[2].result.state).toBe('test');expect(reads[3].result.state).toBe('sustained');
  for(const r of reads.slice(3)){expect(r.state).toBe('blocked');expect(r.reason).toContain('otra vela no verifica el respaldo');expect(r.signals).toEqual([]);}

 }else{
  expect(reads[2].state).toBe('confirmed');expect(final.state).toBe('confirmed');expect(final.signals).toHaveLength(1);
expect(final.signals[0].id).toBe(reads[2].signals[0].id);
 }

});
it('extra closes do not repair partial important-level evidence even with a strong score',()=>{
 const engine=new SessionIntradayRoute();let bars=base.slice(),s=frame(bars);
 const evidence=()=>{const m=market(s);m.levels=[{name:'PUT WALL',role:'put-wall',price:100,strength:85,change:0,observed:start,asof:s.publishedMs,expiration:date,provisional:false,evidence:{quality:'partial'} as NonNullable<import('./route-engine').RouteLevel['evidence']>}];return m;};
 engine.read(s,s.symbol,true,evidence(),s.publishedMs);let result;
 for(const b of [...sequence,bar(11,100.6,100.75,100.5,100.65,150),bar(12,100.65,100.76,100.55,100.66,150)]){
  bars=[...bars,b];s=frame(bars);result=engine.read(s,s.symbol,true,evidence(),s.publishedMs);
 }
 expect(result!.result.state).toBe('sustained');expect(result!.state).toBe('blocked');expect(result!.signals).toEqual([]);
 expect(result!.reason).toContain('otra vela no verifica el respaldo');expect(result!.levelReading).toContain('85.0/100');
});

it('records actual Stage2 veto codes without adding an options availability veto',()=>{
 const ok=run();expect(ok.result.blockCodes).toEqual([]);expect(ok.result.state).toBe('confirmed');
 const missing=run(1,s=>{s.cvd=undefined;});expect(missing.result.state).toBe('blocked');expect(missing.result.blockCodes).toContain('cvd-unconfirmed');expect(missing.result.blockCodes).not.toContain('option-flow-opposed');
 const contrary=run(1,s=>{Object.assign(s.cvd!.recentFive!,{buy:100,sell:800,delta:-700});});expect(contrary.result.blockCodes).toContain('cvd-opposed');
 const m=market(ok.s);m.flow={signal:-.5,coverage:90,at:ok.s.publishedMs,reason:'test'};const blocked=ok.engine.read(ok.s,ok.s.symbol,true,m,ok.s.publishedMs);expect(blocked.blockCodes).toContain('option-flow-opposed');expect(blocked.target).toBe(ok.result.target);
});
it('records remaining geometry and session blockers rather than manufacturing a new signal',()=>{
 const {engine,s,result}=run();s.ticks.AMZN.price=101.9;const near=engine.read(s,s.symbol,true,market(s),s.publishedMs);expect(near.blockCodes).toContain('reward-risk-below-minimum');expect(near.target).toBe(result.target);
 const off=engine.read(s,s.symbol,false,market(s),s.publishedMs);expect(off.blockCodes).toContain('price-unready');expect(off.state).toBe('waiting');expect(off.signals).toEqual([]);
 const late=engine.read(s,s.symbol,true,market(s),s.chartHistory!.sessions[0].closeMs);expect(late.blockCodes).toContain('outside-session');expect(late.blockCodes).toContain('price-unready');expect(late.state).toBe('waiting');
});
