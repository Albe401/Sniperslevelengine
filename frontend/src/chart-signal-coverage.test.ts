import {describe,it,expect} from 'vitest';
import {readChartSignalCoverage,type ChartSignalCoverageKind} from './chart-signal-coverage';
type LiveState=NonNullable<Parameters<typeof readChartSignalCoverage>[0]>;
import type {MarketEvidence} from './market-evidence';
import type {Candle} from './model';

const MINUTE=60000,open=Date.parse('2026-10-26T09:30:00-04:00'),now=open+30*MINUTE,date='2026-10-26';
function source():LiveState {
 const feed={phase:'streaming',events:43,malformed:0,reconnects:0,lastEventMs:now-100,lastReceivedMs:now-50};
 const buckets={'CALL BUY':{contracts:35,premium:350,trades:2},'CALL SELL':{contracts:15,premium:150,trades:1},'PUT BUY':{contracts:20,premium:200,trades:2},'PUT SELL':{contracts:10,premium:100,trades:1},UNKNOWN:{contracts:20,premium:200,trades:2}};
 const period={buy:50,sell:20,unknown:30,delta:30,coveragePct:70,sinceMs:open,lastMs:now-100,gaps:0,changeOne:0,changeFive:30};
 return {symbol:'QQQ',publishedMs:now,startedMs:open,metrics:{session:date,phase:'Sesión RTH'},stocks:{...feed},options:{...feed},ticks:{QQQ:{symbol:'QQQ',price:100,tradeMs:now-200}},flowExpiry:date,
  flow:{asof_ms:now-1000,intraday_windows:[{minutes:5,event_count:8,contracts:100,premium:1000,premium_complete:true,bullish_pct:56.25,bearish_pct:43.75,direction:'BULLISH FLOW',buckets}],capacity_drops:0,rejected_trades:0},
  cvd:{symbol:'QQQ',date,at:now,available:true,regular:{...period},premarket:{...period},rejected:0,recentFive:{buy:50,sell:20,unknown:30,delta:30,startMs:now-300000,endMs:now,complete:true}},
  chartHistory:{sessions:[{date,openMs:open,closeMs:open+390*MINUTE}],minutes:[]},
  optionCaptureCoverage:{method:'option-capture-coverage-v1',requestedChannels:['T.a','Q.a','T.b'],acknowledgedChannels:['T.a','Q.a'],receivedContracts:{a:now-500,b:now-200},connectionStartedMs:open,receptionCoveragePct:null}} as unknown as LiveState;
}
function evidence(s=source()):MarketEvidence{return {symbol:s.symbol,session:`${s.symbol}:${date}:${s.startedMs}`,at:s.publishedMs,flow:{signal:.2,coverage:80,at:s.flow.asof_ms!,reason:'original flow consumer'},cvd:{signal:.3,coverage:70,at:s.cvd!.recentFive!.endMs,reason:'original CVD consumer'}} as MarketEvidence;}
function bars(end=now,length=20):Candle[]{return Array.from({length},(_,i)=>({timestampMs:end-(length-i)*MINUTE,open:100,high:100.1,low:99.9,close:100,volume:10}));}
function read(s=source(),b:readonly Candle[]|undefined=bars(),at=now){return readChartSignalCoverage(s,evidence(s),'QQQ',true,at,{closedMinuteBars:b});}
function row(kind:ChartSignalCoverageKind,s=source()){return read(s).rows.find(r=>r.kind===kind)!;}

describe('chart signal coverage: observed units, source clocks and explicit gaps only',()=>{
 it('separates shares, contracts and USD without changing the existing pressure readings',()=>{
  const s=source(),m=evidence(s),r=readChartSignalCoverage(s,m,'QQQ',true,now,{closedMinuteBars:bars()});
  const stock=r.rows.find(r=>r.kind==='stock-share-classification')!,contracts=r.rows.find(r=>r.kind==='option-contract-classification')!,premium=r.rows.find(r=>r.kind==='option-premium-classification')!;
  expect([stock.unit,stock.numerator,stock.denominator,stock.percent]).toEqual(['shares',70,100,70]);
  expect([contracts.unit,contracts.numerator,contracts.denominator,contracts.percent]).toEqual(['contracts',80,100,80]);
  expect(contracts.complete).toBeNull();expect(contracts.status).toBe('partial');expect(contracts.reasonCodes).toContain('contract-window-completeness-not-reported');
  expect([premium.unit,premium.numerator,premium.denominator,premium.percent]).toEqual(['USD',800,1000,80]);
  expect(r.existingConsumer.flow).toEqual(m.flow);expect(r.existingConsumer.cvd).toEqual(m.cvd);expect(r.existingConsumer.flow).not.toBe(m.flow);
 });
 it.each([0,69.999,70,79.999,80,100])('reports %s percent share classification without creating a strategy gate',pct=>{
  const s=source();Object.assign(s.cvd!.recentFive!,{buy:pct,sell:0,unknown:100-pct,delta:pct});
  const r=row('stock-share-classification',s);expect(r.percent).toBeCloseTo(pct,9);expect(r.status).toBe('available');expect(r.complete).toBe(true);
 });
 it('preserves incomplete share and contract windows while refusing a complete USD denominator',()=>{
  const s=source();s.cvd!.recentFive!.complete=false;s.cvd!.regular.sinceMs=now-40000;s.flow.intraday_windows![0].premium_complete=false;
  const r=read(s),stock=r.rows.find(r=>r.kind==='stock-share-classification')!,contracts=r.rows.find(r=>r.kind==='option-contract-classification')!,premium=r.rows.find(r=>r.kind==='option-premium-classification')!,availability=r.rows.find(r=>r.kind==='option-premium-availability')!;
  expect([stock.percent,stock.complete,stock.status]).toEqual([70,false,'partial']);expect([contracts.percent,contracts.complete,contracts.status]).toEqual([80,null,'partial']);
  expect([premium.numerator,premium.denominator,premium.percent,premium.complete,premium.amountVerifiedUsd]).toEqual([null,null,null,false,1000]);
  expect([availability.numerator,availability.denominator,availability.percent,availability.complete,availability.amountVerifiedUsd]).toEqual([null,null,null,false,1000]);
 });
 it.each(['shares','contracts','premium'])('does not turn an empty %s population into 100 percent',kind=>{
  const s=source();if(kind==='shares')Object.assign(s.cvd!.recentFive!,{buy:0,sell:0,unknown:0,delta:0});
  else{const w=s.flow.intraday_windows![0];for(const b of Object.values(w.buckets!)){b.contracts=0;b.premium=0;b.trades=0;}w.contracts=0;w.premium=0;w.event_count=0;}
  const r=row(kind==='shares'?'stock-share-classification':kind==='contracts'?'option-contract-classification':'option-premium-classification',s);
  expect(r.percent).toBeNull();expect(r.status).toBe('partial');
 });
 it.each(['negative','nonfinite','delta-mismatch','wrong-duration','foreign-cvd','future-window','future-report'])('rejects %s CVD amounts or lineage',kind=>{
  const s=source(),w=s.cvd!.recentFive!;if(kind==='negative')w.unknown=-1;if(kind==='nonfinite')w.buy=NaN;if(kind==='delta-mismatch')w.delta=31;if(kind==='wrong-duration')w.startMs++;if(kind==='foreign-cvd')s.cvd!.symbol='SPY';if(kind==='future-window')w.endMs++;if(kind==='future-report')s.cvd!.at++;
  const r=row('stock-share-classification',s);expect(r.percent).toBeNull();expect(r.numerator).toBeNull();expect(r.denominator).toBeNull();expect(r.status).not.toBe('available');
 });
 it.each(['missing-bucket','negative-contracts','contract-mismatch','trade-count-mismatch','premium-mismatch','future-flow','future-event','wrong-expiry','duplicate-window'])('keeps %s option evidence from a complete diagnostic fraction',kind=>{
  const s=source(),w=s.flow.intraday_windows![0];if(kind==='missing-bucket')delete w.buckets!.UNKNOWN;if(kind==='negative-contracts')w.buckets!.UNKNOWN.contracts=-1;if(kind==='contract-mismatch')w.contracts++;if(kind==='trade-count-mismatch')w.event_count++;if(kind==='premium-mismatch')w.premium++;if(kind==='future-flow')s.flow.asof_ms=now+1;if(kind==='future-event')s.options.lastEventMs=now+1;if(kind==='wrong-expiry')s.flowExpiry='2026-10-27';if(kind==='duplicate-window')s.flow.intraday_windows!.push({...w});
  const r=read(s),target=r.rows.find(r=>r.kind===(kind==='premium-mismatch'?'option-premium-classification':'option-contract-classification'))!;
  expect(target.percent).toBeNull();expect(target.status).not.toBe('available');
 });
 it('does not use next-expiry generic flow windows as a substitute for 0DTE',()=>{
  const s=source();s.flow.windows=s.flow.intraday_windows!;s.flow.intraday_windows=[];
  expect(row('option-contract-classification',s).percent).toBeNull();expect(row('option-premium-classification',s).percent).toBeNull();
 });
 it('retains arithmetic of the observed intraday population with partial context when expiry metadata is absent',()=>{
  const s=source();delete s.flowExpiry;const r=read(s),contracts=r.rows.find(r=>r.kind==='option-contract-classification')!,premium=r.rows.find(r=>r.kind==='option-premium-classification')!;
  expect([contracts.numerator,contracts.denominator,contracts.percent,contracts.status]).toEqual([80,100,80,'partial']);expect(contracts.reasonCodes).toContain('unverified-option-expiry');
  expect(premium.amountVerifiedUsd).toBe(1000);expect(premium.status).toBe('partial');expect('flowExpiry' in s).toBe(false);
 });
 it('rejects fractional option contract quantities without rejecting fractional stock shares',()=>{
  const s=source(),w=s.flow.intraday_windows![0];w.buckets!.UNKNOWN.contracts=20.5;w.contracts=100.5;
  const invalid=row('option-contract-classification',s);expect(invalid.status).toBe('invalid');expect(invalid.percent).toBeNull();expect(invalid.reasonCodes).toContain('invalid-option-contract-quantity');
  Object.assign(s.cvd!.recentFive!,{buy:.25,sell:.5,unknown:.25,delta:-.25});const stock=row('stock-share-classification',s);expect(stock.percent).toBe(75);expect(stock.status).toBe('available');
 });
 it('derives the options bounds only from original flow asof and retains CVD bounds exactly',()=>{
  const s=source(),r=read(s);const cvd=r.rows.find(r=>r.kind==='stock-share-classification')!,flow=r.rows.find(r=>r.kind==='option-contract-classification')!;
  expect(cvd.window).toMatchObject({startMs:s.cvd!.recentFive!.startMs,endMs:s.cvd!.recentFive!.endMs});
  expect(flow.window).toMatchObject({startMs:s.flow.asof_ms!-300000,endMs:s.flow.asof_ms});expect(flow.freshness.publicationLagMs).toBe(1000);
  expect(read(s,undefined,now+1000).rows.find(r=>r.kind==='option-contract-classification')!.window).toEqual(flow.window);
 });
 it('flags stale source and publication clocks without replacing them by the evaluation clock',()=>{
  const s=source(),r=read(s,bars(),now+5000),flow=r.rows.find(r=>r.kind==='option-contract-classification')!;
  expect(r.publishedMs).toBe(now);expect(r.evaluatedMs).toBe(now+5000);expect(flow.freshness.sourceAtMs).toBe(now-1000);expect(flow.freshness.sourceAgeMs).toBe(6000);expect(flow.status).toBe('partial');expect(r.existingConsumer.flow).toBeNull();
 });
 it.each(['source','publication','symbol','session','inactive','evaluation','connection'])('does not synthesize scoped measurements for invalid %s',kind=>{
  const s=source();let at=now,active=true;if(kind==='publication')s.publishedMs++;if(kind==='symbol')s.symbol='SPY';if(kind==='session')s.metrics.session='2026-10-25';if(kind==='inactive')active=false;if(kind==='evaluation')at=NaN;if(kind==='connection')s.startedMs=now+1;
  const r=readChartSignalCoverage(kind==='source'?undefined:s,evidence(s),'QQQ',active,at,{closedMinuteBars:bars()});
  for(const entry of r.rows){expect(entry.numerator).toBeNull();expect(entry.denominator).toBeNull();expect(entry.percent).toBeNull();}
  expect(r.existingConsumer.flow).toBeNull();if(kind==='evaluation')expect(r.evaluatedMs).toBeNull();
 });
 it('keeps source corrections explicit at their actual publication without mutating former evidence',()=>{
  const s=source(),original=read(s),saved=JSON.stringify(original);s.publishedMs+=100;Object.assign(s.cvd!.recentFive!,{buy:60,unknown:20,delta:40});s.cvd!.at+=100;
  const corrected=read(s,bars(),now+100);expect(corrected.publishedMs).toBe(now+100);expect(corrected.rows.find(r=>r.kind==='stock-share-classification')!.percent).toBe(80);expect(JSON.stringify(original)).toBe(saved);
 });
 it('does not invent denominator coverage from a truncated preexisting MarketEvidence reading',()=>{
  const s=source(),m=evidence(s);delete s.cvd;delete s.flow.intraday_windows;
  const r=readChartSignalCoverage(s,m,'QQQ',true,now);expect(r.rows.find(r=>r.kind==='stock-share-classification')!.denominator).toBeNull();expect(r.rows.find(r=>r.kind==='option-contract-classification')!.denominator).toBeNull();expect(r.existingConsumer.flow).toEqual(m.flow);
 });
 it('never interprets feed counters, channel acknowledgements or contract timestamps as market reception percent',()=>{
  const s=source(),r=read(s);for(const entry of r.rows.filter(r=>r.kind==='stock-reception'||r.kind==='option-reception')){expect(entry.numerator).toBeNull();expect(entry.denominator).toBeNull();expect(entry.percent).toBeNull();expect(entry.complete).toBeNull();}
  expect(r.feedDiagnostics.stocks).toMatchObject({events:43,scope:'connection-feed-not-asset-window'});expect(r.feedDiagnostics.optionCapture).toMatchObject({requestedChannels:3,acknowledgedChannels:2,contractsWithEvents:2,lastContractEventMs:now-200,receptionPercent:null});
 });
 it('rejects foreign or future received-contract timestamps instead of summing them as counts',()=>{
  const s=source();s.optionCaptureCoverage!.receivedContracts.a=now+1;expect(read(s).feedDiagnostics.optionCapture!.contractsWithEvents).toBeNull();
  s.optionCaptureCoverage!.receivedContracts={foreign:now};expect(read(s).feedDiagnostics.optionCapture!.contractsWithEvents).toBeNull();
 });
 it('keeps price-clock percentage and complete universe null while reporting original trade age',()=>{
  const r=row('price-clock');expect([r.numerator,r.denominator,r.percent,r.complete]).toEqual([null,null,null,null]);expect(r.freshness.sourceAtMs).toBe(now-200);expect(r.freshness.sourceAgeMs).toBe(200);expect(r.status).toBe('available');
 });
 it('counts only valid unique closed minute slots in the last at-most twenty calendar minutes',()=>{
  const r=read().rows.find(r=>r.kind==='closed-minute-continuity')!;expect([r.numerator,r.denominator,r.percent,r.complete]).toEqual([20,20,100,true]);expect(r.window).toMatchObject({startMs:now-20*MINUTE,endMs:now,durationMs:20*MINUTE});
  const b=bars(),duplicate=read(source(),[...b,b[0]]).rows.find(r=>r.kind==='closed-minute-continuity')!;expect(duplicate.numerator).toBe(20);expect(duplicate.percent).toBe(100);expect(duplicate.reasonCodes).toContain('duplicate-minute-not-double-counted');
 });
 it('reports missing candle input as unknown, and only an explicit empty expected window as zero',()=>{
  const unknown=readChartSignalCoverage(source(),evidence(),'QQQ',true,now).rows.find(r=>r.kind==='closed-minute-continuity')!;
  expect([unknown.numerator,unknown.denominator,unknown.percent,unknown.complete]).toEqual([null,null,null,null]);
  const empty=read(source(),[]).rows.find(r=>r.kind==='closed-minute-continuity')!;expect([empty.numerator,empty.denominator,empty.percent,empty.complete]).toEqual([0,20,0,false]);expect(empty.status).toBe('partial');
 });
 it('does not backdate a minute that closed after the original publication',()=>{
  const s=source();s.publishedMs=now-1;const r=read(s).rows.find(r=>r.kind==='closed-minute-continuity')!;
  expect(r.window.endMs).toBe(now-MINUTE);expect(r.numerator).toBe(19);expect(r.denominator).toBe(20);expect(r.complete).toBe(false);
 });
 it('keeps missing and conflicting corrected minutes partial or invalid without fabricating candles',()=>{
  const b=bars(),missing=read(source(),b.slice(1)).rows.find(r=>r.kind==='closed-minute-continuity')!;expect(missing.numerator).toBe(19);expect(missing.complete).toBe(false);
  const conflicting=read(source(),[...b,{...b[0],volume:11}]).rows.find(r=>r.kind==='closed-minute-continuity')!;expect(conflicting.status).toBe('invalid');expect(conflicting.numerator).toBeNull();expect(conflicting.percent).toBeNull();
 });
 it('honors early calendar closes and does not make an expected minute before the first session close',()=>{
  const s=source();s.chartHistory!.sessions[0].closeMs=now-5*MINUTE;const early=read(s).rows.find(r=>r.kind==='closed-minute-continuity')!;expect(early.window.endMs).toBe(now-5*MINUTE);expect(early.denominator).toBe(20);expect(early.numerator).toBe(15);
  const first=source();first.publishedMs=open+MINUTE-1;const pending=read(first,[],open+MINUTE-1).rows.find(r=>r.kind==='closed-minute-continuity')!;expect(pending.numerator).toBeNull();expect(pending.denominator).toBeNull();expect(pending.percent).toBeNull();
 });
 it('does not re-read raw history or use a new source clock to count the supplied candles',()=>{
  const s=source();Object.defineProperty(s.chartHistory!,'minutes',{get(){throw new Error('raw history must not be read');}});
  expect(read(s).rows.find(r=>r.kind==='closed-minute-continuity')!.numerator).toBe(20);
 });
 it('does not mutate caller values or expose mutable reports',()=>{
  const s=source(),m=evidence(s),b=bars(),saved=JSON.stringify([s,m,b]),r=readChartSignalCoverage(s,m,'QQQ',true,now,{closedMinuteBars:b});
  expect(JSON.stringify([s,m,b])).toBe(saved);expect(Object.isFrozen(r.rows[0].freshness)).toBe(true);expect(Object.isFrozen(r.existingConsumer.flow)).toBe(true);expect(Object.isFrozen(s)).toBe(false);expect(Object.isFrozen(b[0])).toBe(false);
  expect('probability' in r).toBe(false);expect('executionEnabled' in r).toBe(false);expect('availableMs' in r).toBe(false);
 });
});
