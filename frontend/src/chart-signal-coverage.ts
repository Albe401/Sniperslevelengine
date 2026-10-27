import type {LiveState} from './live';
import type {Candle} from './model';
import type {MarketEvidence,PressureEvidence} from './market-evidence';

type CapturedCoverageState=LiveState&{optionCaptureCoverage?:{method:string;requestedChannels:string[];acknowledgedChannels:string[];receivedContracts:Record<string,number>;connectionStartedMs:number;receptionCoveragePct:null}};

export type ChartSignalCoverageKind='price-clock'|'closed-minute-continuity'|'stock-share-classification'|'option-contract-classification'|'option-premium-classification'|'option-premium-availability'|'stock-reception'|'option-reception';
export type ChartSignalCoverageStatus='available'|'partial'|'unavailable'|'invalid';
export interface ChartSignalCoverageWindow {
 startMs:number|null;endMs:number|null;durationMs:number|null;
 boundsBasis:'tick-time'|'calendar-closed-minutes'|'source-recent-five'|'flow-asof-minus-duration'|'feed-connection'|'unavailable';
}
export interface ChartSignalCoverageFreshness {
 sourceAtMs:number|null;sourceAgeMs:number|null;publishedAgeMs:number|null;publicationLagMs:number|null;
 lastEventMs:number|null;eventAgeMs:number|null;
}
export interface ChartSignalCoverageRow {
 kind:ChartSignalCoverageKind;unit:'shares'|'contracts'|'USD'|'events'|'minutes';sourcePath:string;
 numerator:number|null;denominator:number|null;percent:number|null;
 /** Original source marker, except for the explicitly counted closed-minute row; not market reception. */
 complete:boolean|null;
 status:ChartSignalCoverageStatus;window:ChartSignalCoverageWindow;freshness:ChartSignalCoverageFreshness;
 reasonCodes:readonly string[];amountVerifiedUsd?:number|null;
}
export interface ChartSignalCoverageFeedFacts {
 scope:'connection-feed-not-asset-window';phase:string|null;events:number|null;malformed:number|null;reconnects:number|null;
 lastEventMs:number|null;lastReceivedMs:number|null;
}
export interface ChartSignalCoverageReport {
 version:'chart-signal-coverage-v1';symbol:string;sessionKey:string|null;evaluatedMs:number|null;publishedMs:number|null;connectionStartedMs:number|null;
 reasonCodes:readonly string[];rows:readonly ChartSignalCoverageRow[];
 feedDiagnostics:{
  stocks:ChartSignalCoverageFeedFacts|null;options:ChartSignalCoverageFeedFacts|null;
  optionCapture:{method:string|null;requestedChannels:number|null;acknowledgedChannels:number|null;contractsWithEvents:number|null;lastContractEventMs:number|null;connectionStartedMs:number|null;receptionPercent:null;reasonCodes:readonly string[]}|null;
  flow:{scope:'flow-engine-cumulative-not-window';capacityDrops:number|null;rejectedTrades:number|null}|null;
  cvd:{scope:'observed-segment-not-window';rejected:number|null;regularGaps:number|null;regularSinceMs:number|null}|null;
 };
 existingConsumer:{flow:Readonly<PressureEvidence>|null;cvd:Readonly<PressureEvidence>|null};
}
export interface ChartSignalCoverageInput {closedMinuteBars?:readonly Candle[]}
const MINUTE=60000,FIVE=300000,LABELS=['CALL BUY','CALL SELL','PUT BUY','PUT SELL','UNKNOWN'] as const;
const clock=(n:unknown):n is number=>typeof n==='number'&&Number.isSafeInteger(n)&&n>0&&n<=8640000000000000;
const finite=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n);
const quantity=(n:unknown):n is number=>finite(n)&&n>=0;
const count=(n:unknown):n is number=>quantity(n)&&Number.isSafeInteger(n);
const countOrNull=(n:unknown)=>count(n)?n:null;
const clockOrNull=(n:unknown)=>clock(n)?n:null;
const equal=(a:number,b:number)=>Math.abs(a-b)<=Math.abs(b)*1e-8;
const et=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
const etTime=new Intl.DateTimeFormat('en-GB',{timeZone:'America/New_York',hourCycle:'h23',hour:'2-digit',minute:'2-digit'});
const windowMissing=():ChartSignalCoverageWindow=>({startMs:null,endMs:null,durationMs:null,boundsBasis:'unavailable'});
const unique=(codes:readonly string[])=>[...new Set(codes)];
function freeze<T>(value:T):Readonly<T>{if(value&&typeof value==='object'&&!Object.isFrozen(value)){for(const child of Object.values(value))freeze(child);Object.freeze(value);}return value;}
function feedFacts(feed:LiveState['stocks']|undefined):ChartSignalCoverageFeedFacts|null {
 return feed?{scope:'connection-feed-not-asset-window',phase:typeof feed.phase==='string'?feed.phase:null,events:countOrNull(feed.events),malformed:countOrNull(feed.malformed),reconnects:countOrNull(feed.reconnects),lastEventMs:clockOrNull(feed.lastEventMs),lastReceivedMs:clockOrNull(feed.lastReceivedMs)}:null;
}
function pressure(p:PressureEvidence|undefined):Readonly<PressureEvidence>|null {
 if(!p||!(p.signal===null||finite(p.signal)&&Math.abs(p.signal)<=1)||!(p.coverage===null||quantity(p.coverage)&&p.coverage<=100)||!(p.at===null||clock(p.at))||typeof p.reason!=='string')return null;
 return {signal:p.signal,coverage:p.coverage,at:p.at,reason:p.reason};
}

/** Diagnostic arithmetic only. Source completeness is preserved, never inferred from a percentage.
 * All percentages describe the stated observed population, never market reception or probability.
 * Existing strategies retain their own coverage thresholds, candle minimums, RR and decisions.
 * supplied closedMinuteBars are already computed by the caller; raw chart history is never rebuilt.
 */
export function readChartSignalCoverage(s:CapturedCoverageState|undefined,market:MarketEvidence|undefined,symbol:string,active:boolean,now:number,input:ChartSignalCoverageInput={}):Readonly<ChartSignalCoverageReport> {
 const published=clockOrNull(s?.publishedMs),started=clockOrNull(s?.startedMs),date=clock(now)?et.format(now):null;
 const reasons:string[]=[];
 if(!clock(now))reasons.push('invalid-evaluation-clock');
 if(!active)reasons.push('inactive');
 if(!s)reasons.push('missing-source');else if(s.symbol!==symbol)reasons.push('foreign-symbol');
 if(published===null)reasons.push('missing-publication-clock');else if(clock(now)&&published>now)reasons.push('future-publication');
 if(published!==null&&started!==null&&started>published)reasons.push('future-connection-clock');
 if(s&&date&&s.metrics?.session!==date)reasons.push('foreign-session');
 const own=!!s&&s.symbol===symbol,scoped=own&&active&&clock(now)&&published!==null&&published<=now&&(started===null||started<=published)&&s.metrics?.session===date;
 const sessionKey=scoped&&started!==null?`${symbol}:${date}:${started}`:null;
 if(scoped&&started===null)reasons.push('missing-connection-clock');
 if(scoped&&s.metrics?.phase!=='Sesión RTH')reasons.push('outside-rth');
 if(scoped&&now-published!>3000)reasons.push('stale-publication');
 const freshness=(source:unknown,event:unknown):ChartSignalCoverageFreshness=>{
  const sourceAtMs=clockOrNull(source),lastEventMs=clockOrNull(event);
  return {sourceAtMs,sourceAgeMs:sourceAtMs!==null&&clock(now)&&sourceAtMs<=now?now-sourceAtMs:null,publishedAgeMs:published!==null&&clock(now)&&published<=now?now-published:null,publicationLagMs:sourceAtMs!==null&&published!==null&&sourceAtMs<=published?published-sourceAtMs:null,lastEventMs,eventAgeMs:lastEventMs!==null&&clock(now)&&lastEventMs<=now?now-lastEventMs:null};
 };
 const base=(kind:ChartSignalCoverageKind,unit:ChartSignalCoverageRow['unit'],path:string):ChartSignalCoverageRow=>({kind,unit,sourcePath:path,numerator:null,denominator:null,percent:null,complete:null,status:reasons.some(c=>c.startsWith('invalid-')||c.startsWith('future-'))?'invalid':'unavailable',window:windowMissing(),freshness:freshness(null,null),reasonCodes:reasons.slice()});
 const add=(r:ChartSignalCoverageRow,codes:readonly string[],invalid=false)=>{r.reasonCodes=unique([...r.reasonCodes,...codes]);if(invalid)r.status='invalid';};
 const temporal=(r:ChartSignalCoverageRow,source:unknown,event:unknown,maxSourceAge=3000):boolean=>{
  r.freshness=freshness(source,event);
  if(!clock(source)){add(r,['missing-source-clock']);return false;}
  if(published===null||source>published||!clock(now)||source>now){add(r,['future-source-data'],true);return false;}
  if(now-source>maxSourceAge)add(r,['stale-source-data']);
  if(!clock(event))add(r,['missing-last-event-clock']);
  else if(event>published||event>now){add(r,['future-last-event'],true);return false;}
  else if(now-event>15000)add(r,['stale-last-event']);
  return true;
 };
 const finishStatus=(r:ChartSignalCoverageRow,complete:boolean|null)=>{
  r.complete=complete;
  if(r.status!=='invalid')r.status=complete===false||r.reasonCodes.length>0?'partial':'available';
 };
 const price=base('price-clock','events','ticks[symbol].tradeMs');
 if(scoped){
  const tick=s.ticks?.[symbol];
  if(!tick||tick.symbol!==symbol)add(price,['missing-own-price']);
  else if(!finite(tick.price)||tick.price<=0)add(price,['invalid-price'],true);
  else if(temporal(price,tick.tradeMs,tick.tradeMs,15000)){
   price.window={startMs:tick.tradeMs!,endMs:tick.tradeMs!,durationMs:null,boundsBasis:'tick-time'};
   if(s.stocks?.phase!=='streaming')add(price,['stock-feed-not-streaming']);
   finishStatus(price,null);
  }
 }

 const minutes=base('closed-minute-continuity','minutes','input.closedMinuteBars + chartHistory.sessions calendar');
 if(scoped){
  const sessions=s.chartHistory?.sessions?.filter(v=>v.date===date)??[],calendar=sessions.length===1?sessions[0]:null;
  if(!calendar||!clock(calendar.openMs)||!clock(calendar.closeMs)||calendar.closeMs<=calendar.openMs)add(minutes,['missing-unique-session-calendar']);
  else if(!Array.isArray(input.closedMinuteBars))add(minutes,['missing-closed-minute-input']);
  else{
   const through=calendar.openMs+Math.floor((Math.min(published!,calendar.closeMs)-calendar.openMs)/MINUTE)*MINUTE;
   if(through<=calendar.openMs)add(minutes,['no-closed-rth-minute']);
   else{
    const from=Math.max(calendar.openMs,through-20*MINUTE),expected=(through-from)/MINUTE,seen=new Map<number,Candle>();let invalid=false,duplicates=false;
    minutes.window={startMs:from,endMs:through,durationMs:through-from,boundsBasis:'calendar-closed-minutes'};minutes.freshness=freshness(through,s.ticks?.[symbol]?.tradeMs);
    for(const b of input.closedMinuteBars){
     if(!b||!clock(b.timestampMs)){invalid=true;continue;}
     if(b.timestampMs<from||b.timestampMs>=through)continue;
     if((b.timestampMs-calendar.openMs)%MINUTE!==0||![b.open,b.high,b.low,b.close,b.volume].every(finite)||b.low<=0||b.volume<0||b.high<Math.max(b.open,b.close)||b.low>Math.min(b.open,b.close)){invalid=true;continue;}
     const old=seen.get(b.timestampMs);
     if(old){duplicates=true;if(old.open!==b.open||old.high!==b.high||old.low!==b.low||old.close!==b.close||old.volume!==b.volume)invalid=true;}
     else seen.set(b.timestampMs,b);
    }
    if(invalid)add(minutes,['invalid-or-conflicting-closed-minute'],true);
    else{
     minutes.numerator=seen.size;minutes.denominator=expected;minutes.percent=100*seen.size/expected;
     if(seen.size!==expected)add(minutes,['missing-closed-minutes']);
     // Identical repeated rows count once and cannot increase completeness.
     if(duplicates)add(minutes,['duplicate-minute-not-double-counted']);
     finishStatus(minutes,seen.size===expected);
    }
   }
  }
 }

 const stock=base('stock-share-classification','shares','cvd.recentFive');
 if(scoped){
  const c=s.cvd,w=c?.recentFive,p=c?.regular;
  if(!c||c.symbol!==symbol||c.date!==date||!w)add(stock,['missing-own-cvd-window']);
  else{
   stock.complete=typeof w.complete==='boolean'?w.complete:null;
   if(!clock(w.startMs)||!clock(w.endMs)||w.endMs-w.startMs!==FIVE||et.format(w.startMs)!==date)add(stock,['invalid-cvd-window'],true);
   else if(temporal(stock,w.endMs,p?.lastMs)&&clock(c.at)&&c.at<=published!&&c.at>=w.endMs){
    stock.window={startMs:w.startMs,endMs:w.endMs,durationMs:FIVE,boundsBasis:'source-recent-five'};
    const total=w.buy+w.sell+w.unknown;
    if(![w.buy,w.sell,w.unknown,total].every(quantity)||!finite(w.delta)||Math.abs(w.delta-w.buy+w.sell)>Math.max(1e-6,Math.abs(w.delta)*1e-9))add(stock,['invalid-cvd-amounts-or-delta'],true);
    else{
     stock.numerator=w.buy+w.sell;stock.denominator=total;stock.percent=total>0?100*stock.numerator/total:null;
     if(total===0)add(stock,['empty-observed-share-population']);
     if(w.complete!==true)add(stock,['incomplete-cvd-window']);
     if(c.available!==true||s.stocks?.phase!=='streaming')add(stock,['cvd-not-currently-available']);
     if(!clock(p?.sinceMs)||p!.sinceMs!>w.startMs)add(stock,['cvd-segment-start-not-covering-window']);
     const parts=etTime.format(w.startMs);
     if(parts<'09:30')add(stock,['cvd-window-before-rth']);
     finishStatus(stock,stock.complete);
    }
   }else if(stock.status!=='invalid')add(stock,['invalid-cvd-report-clock'],true);
  }
 }

 const contracts=base('option-contract-classification','contracts','flow.intraday_windows[minutes=5].buckets');
 const premium=base('option-premium-classification','USD','flow.intraday_windows[minutes=5].buckets');
 const availability=base('option-premium-availability','events','flow.intraday_windows[minutes=5].premium_complete');
 if(scoped){
  const flow=s.flow,windows=flow?.intraday_windows?.filter(w=>w.minutes===5)??[],w=windows.length===1?windows[0]:null;
  if(!w)for(const r of [contracts,premium,availability])add(r,['missing-unique-five-minute-option-window']);
  else{
   const flowAt=flow.asof_ms,optionsAt=s.options?.lastEventMs,missingExpiry=s.flowExpiry==null,foreignExpiry=!missingExpiry&&s.flowExpiry!==date;
   for(const r of [contracts,premium,availability]){
    r.complete=r===contracts?null:typeof w.premium_complete==='boolean'?w.premium_complete:null;
    if(r===contracts)add(r,['contract-window-completeness-not-reported']);
    if(missingExpiry)add(r,['unverified-option-expiry']);
    else if(foreignExpiry)add(r,['foreign-option-expiry'],true);
    if(s.options?.phase!=='streaming')add(r,['option-feed-not-streaming']);
    if(clock(flowAt)&&flowAt>=FIVE)r.window={startMs:flowAt-FIVE,endMs:flowAt,durationMs:FIVE,boundsBasis:'flow-asof-minus-duration'};
    temporal(r,flowAt,optionsAt);
   }
   const scopeValid=!foreignExpiry&&clock(flowAt)&&flowAt>=FIVE&&flowAt<=published!&&flowAt<=now&&et.format(flowAt-FIVE)===date&&[contracts,premium,availability].every(r=>r.status!=='invalid');
   if(!scopeValid){for(const r of [contracts,premium,availability])add(r,['invalid-option-window-scope'],true);}
   else{
    const buckets=LABELS.map(k=>w.buckets?.[k]),contractAmounts=buckets.map(b=>b?.contracts),premiums=buckets.map(b=>b?.premium),trades=buckets.map(b=>b?.trades);
    // Massive Options WS documents s as integer. Diagnostic validation only; strategies are unchanged.
    if(contractAmounts.every(count)&&count(w.contracts)){
     const amounts=contractAmounts as number[],total=amounts.reduce((a,b)=>a+b,0),known=amounts.slice(0,4).reduce((a,b)=>a+b,0);
     if(!count(total)||total!==w.contracts)add(contracts,['contract-bucket-total-mismatch'],true);
     else if(!count(w.event_count)||!trades.every(count)||(trades as number[]).reduce((a,b)=>a+b,0)!==w.event_count)add(contracts,['option-trade-count-mismatch'],true);
     else{
      contracts.numerator=known;contracts.denominator=total;contracts.percent=total>0?100*known/total:null;
      if(total===0)add(contracts,['empty-observed-contract-population']);
      if(w.premium_complete!==true)add(contracts,['option-premium-source-incomplete']);
      finishStatus(contracts,contracts.complete);
     }
    }else add(contracts,[contractAmounts.every(quantity)&&quantity(w.contracts)?'invalid-option-contract-quantity':'missing-or-invalid-contract-buckets'],true);
    if(premiums.every(quantity)&&quantity(w.premium)){
     const amounts=premiums as number[],total=amounts.reduce((a,b)=>a+b,0),known=amounts.slice(0,4).reduce((a,b)=>a+b,0);
     if(!finite(total)||!equal(total,w.premium)){for(const r of [premium,availability])add(r,['premium-bucket-total-mismatch'],true);}
     else{
      premium.amountVerifiedUsd=total;availability.amountVerifiedUsd=total;
      if(w.premium_complete!==true){add(premium,['incomplete-premium-population']);add(availability,['source-window-completeness-not-certified']);}
      else if(!count(w.event_count)||w.event_count===0){add(premium,['empty-observed-premium-population']);add(availability,['empty-observed-premium-population']);}
      else if(total<=0){add(premium,['empty-observed-premium-population']);}
      else{premium.numerator=known;premium.denominator=total;premium.percent=100*known/total;}
      finishStatus(premium,premium.complete);finishStatus(availability,availability.complete);
     }
    }else for(const r of [premium,availability])add(r,['missing-or-invalid-premium-buckets'],true);
    add(availability,['priced-execution-denominator-not-reported']);
    if(availability.status==='available')availability.status='partial';
   }
  }
 }

 const stockReception=base('stock-reception','events','stocks (connection-level feed counters)');
 const optionReception=base('option-reception','events','optionCaptureCoverage + options (connection-level facts)');
 let capture:ChartSignalCoverageReport['feedDiagnostics']['optionCapture']=null;
 if(scoped){
  for(const [row,feed] of [[stockReception,s.stocks],[optionReception,s.options]] as const){
   row.freshness=freshness(feed?.lastEventMs,feed?.lastEventMs);
   row.window={startMs:started,endMs:published,durationMs:started!==null&&published!>=started?published!-started:null,boundsBasis:'feed-connection'};
   add(row,['market-reception-denominator-not-known','feed-counters-are-not-asset-window-trade-counts']);
  }
  const c=s.optionCaptureCoverage;
  if(c){
   const codes:string[]=['market-reception-denominator-not-known'],validStrings=(v:unknown):v is string[]=>Array.isArray(v)&&v.every(x=>typeof x==='string'&&x.length>0)&&new Set(v).size===v.length;
   let requested:number|null=null,acknowledged:number|null=null,received:number|null=null,last:number|null=null;
   const requestedSet=validStrings(c.requestedChannels)?new Set(c.requestedChannels):null;
   if(requestedSet&&validStrings(c.acknowledgedChannels)&&c.acknowledgedChannels.every(k=>requestedSet.has(k))){requested=c.requestedChannels.length;acknowledged=c.acknowledgedChannels.length;}else codes.push('invalid-channel-acknowledgements');
   const rows=c.receivedContracts&&typeof c.receivedContracts==='object'?Object.entries(c.receivedContracts):null;
   if(rows&&requestedSet&&requested!==null&&rows.every(([ticker,at])=>clock(at)&&at<=published!&&at<=now&&(requestedSet.has('T.'+ticker)||requestedSet.has('Q.'+ticker)))){received=rows.length;last=rows.reduce<number|null>((max,[,at])=>max===null?at:Math.max(max,at),null);}else codes.push('invalid-contract-last-event-map');
   capture={method:typeof c.method==='string'?c.method:null,requestedChannels:requested,acknowledgedChannels:acknowledged,contractsWithEvents:received,lastContractEventMs:last,connectionStartedMs:clockOrNull(c.connectionStartedMs),receptionPercent:null,reasonCodes:codes};
   add(optionReception,codes);
  }else add(optionReception,['missing-option-capture-facts']);
 }
 const marketComparable=scoped&&sessionKey!==null&&market?.symbol===symbol&&market.session===sessionKey&&clock(market.at)&&market.at<=now&&now-market.at<=3000;
 return freeze({version:'chart-signal-coverage-v1' as const,symbol,sessionKey,evaluatedMs:clockOrNull(now),publishedMs:published,connectionStartedMs:started,reasonCodes:unique(reasons),rows:[price,minutes,stock,contracts,premium,availability,stockReception,optionReception],
  feedDiagnostics:{stocks:scoped?feedFacts(s.stocks):null,options:scoped?feedFacts(s.options):null,optionCapture:capture,flow:scoped&&s.flow?{scope:'flow-engine-cumulative-not-window' as const,capacityDrops:countOrNull(s.flow.capacity_drops),rejectedTrades:countOrNull(s.flow.rejected_trades)}:null,cvd:scoped&&s.cvd?.symbol===symbol&&s.cvd.date===date?{scope:'observed-segment-not-window' as const,rejected:countOrNull(s.cvd.rejected),regularGaps:countOrNull(s.cvd.regular?.gaps),regularSinceMs:clockOrNull(s.cvd.regular?.sinceMs)}:null},
  existingConsumer:{flow:marketComparable?pressure(market.flow):null,cvd:marketComparable?pressure(market.cvd):null}});
}
