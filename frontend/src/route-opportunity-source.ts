import {ROUTE_SEQUENCE_WINDOW_MS,routeSequenceLastAllowedBarStartMs} from './route-sequence-policy';
import type {Candle} from './model';
import type {RouteResult,RouteSequenceSnapshot} from './route-engine';
import type {MarketSignal} from './market-signals';

export type RouteOpportunityStrategy='important-level-route-v1'|'session-intraday-v1';
export interface OpportunityBar {startMs:number;endMs:number;close:number}
export interface FrozenRouteOpportunity {
 version:'frozen-route-opportunity-v1';strategyId:RouteOpportunityStrategy;sourceResultKey:string;sourceState:'sustained'|'confirmed';direction:1|-1;
 origin:{symbol:string;session:string;publishedMs:number;startedMs:number;evaluatedMs:number;marketEvidenceKey:string};
 signal:{id:string;session:string;availableMs:number;evidenceKey:string};
 triggerBar:OpportunityBar;impulseBar:OpportunityBar;continuationBar:OpportunityBar|null;
 anchor:{name:string;price:number;expiration:string;firstObservedMs:number;asofMs:number|null;source:string|null};
 target:{price:number;name:string|null;expiration:string|null;selectedMs:number|null;firstObservedMs:number|null;asofMs:number|null;source:string|null};
 stop:number;expiry:{clock:'bar-start';lastAllowedBarStartMs:number;elapsedMs:600000;comparison:'>'};
}
export interface RouteOpportunitySource {
 version:'route-opportunity-source-v1';symbol:string;publishedMs:number|null;startedMs:number|null;at:number;
 market:{symbol:string;session:string;key:string;at:number};
 strategies:{strategyId:RouteOpportunityStrategy;current:{state:string;resultState:RouteResult['state'];reason:string;key:string;direction:number;level:number|null;target:number|null;stop:number|null;evaluatedMs:number;resultAt:number;lastClosedBar:OpportunityBar|null};event:FrozenRouteOpportunity|null}[];
}
export interface RouteOpportunityInput {
 symbol:string;publishedMs:number|null;startedMs:number|null;at:number;market:RouteOpportunitySource['market'];bars:readonly Candle[];
 strategies:{strategyId:RouteOpportunityStrategy;state:string;reason?:string;result:RouteResult;signals:readonly MarketSignal[];sequence:RouteSequenceSnapshot|undefined;lastClosedBar:OpportunityBar|null}[];
}
const clock=(n:unknown):n is number=>typeof n==='number'&&Number.isSafeInteger(n)&&n>0&&n<=8640000000000000;
const positive=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n)&&n>0;
function immutable<T>(value:T):Readonly<T>{if(value&&typeof value==='object'&&!Object.isFrozen(value)){for(const child of Object.values(value))immutable(child);Object.freeze(value);}return value;}
/** Metadata only: capture a newly emitted source event, never reconstruct a skipped old signal. */
function capture(input:RouteOpportunityInput,v:RouteOpportunityInput['strategies'][number]):FrozenRouteOpportunity|null {
 const {result:r,sequence:q}=v;
 if(!q||q.key!==r.key||q.phase!=='sustained'||!r.key||r.state!=='sustained'||![1,-1].includes(r.direction)||q.direction!==r.direction||![r.level,r.target,r.stop].every(positive))return null;
 if(v.strategyId==='session-intraday-v1'?v.state!=='confirmed':v.state!=='sustained')return null;
 const signal=v.signals.find(s=>s.state==='sustained'&&(v.strategyId==='session-intraday-v1'?s.id===r.key+':confirmed':s.id.startsWith(r.key+':sustained:')));
 if(!signal||signal.direction!==r.direction||signal.session!==input.market.session||!signal.id||!signal.evidenceKey||!clock(signal.availableMs)||signal.availableMs!==input.at||!clock(input.publishedMs)||!clock(input.startedMs)||input.startedMs>input.publishedMs||input.publishedMs>input.at)return null;
 const bar=input.bars.find(b=>b.timestampMs===signal.barMs);
 if(!bar||!clock(bar.timestampMs)||bar.timestampMs+60000>signal.availableMs||bar.timestampMs+60000>input.publishedMs||!positive(bar.close)||!clock(q.impulse.startMs)||q.impulse.endMs!==q.impulse.startMs+60000||q.impulse.endMs>bar.timestampMs+60000||!positive(q.impulse.close))return null;
 if(!positive(q.anchor.price)||q.anchor.price!==r.level||!clock(q.anchor.firstObservedMs)||q.anchor.firstObservedMs>q.impulse.startMs||q.anchor.asofMs!==null&&(!clock(q.anchor.asofMs)||q.anchor.asofMs>signal.availableMs))return null;
 return {
  version:'frozen-route-opportunity-v1',strategyId:v.strategyId,sourceResultKey:r.key,sourceState:v.strategyId==='session-intraday-v1'?'confirmed':'sustained',direction:r.direction as 1|-1,
  origin:{symbol:input.symbol,session:input.market.session,publishedMs:input.publishedMs,startedMs:input.startedMs,evaluatedMs:input.at,marketEvidenceKey:input.market.key},
  signal:{id:signal.id,session:signal.session,availableMs:signal.availableMs,evidenceKey:signal.evidenceKey},
  triggerBar:{startMs:bar.timestampMs,endMs:bar.timestampMs+60000,close:bar.close},impulseBar:{...q.impulse},continuationBar:q.continuation?{...q.continuation}:null,
  anchor:{...q.anchor},target:{price:r.target!,name:q.destination?.name??null,expiration:q.destination?.expiration??null,selectedMs:q.destination?.selectedMs??null,firstObservedMs:q.destination?.firstObservedMs??null,asofMs:q.destination?.asofMs??null,source:q.destination?.source??null},
  stop:r.stop!,expiry:{clock:'bar-start',lastAllowedBarStartMs:routeSequenceLastAllowedBarStartMs(q.impulse.startMs),elapsedMs:ROUTE_SEQUENCE_WINDOW_MS,comparison:'>'}
 };
}
/** Two bounded records, source geometry frozen once. Reading never advances a trading engine. */
export class RouteOpportunityRecorder {
 private identity='';private lastAt=0;private cache=new Map<RouteOpportunityStrategy,FrozenRouteOpportunity>();private latest?:Readonly<RouteOpportunitySource>;
 observe(input:RouteOpportunityInput):void {
  const identity=JSON.stringify([input.symbol,input.market.session,input.startedMs]);
  if(identity!==this.identity||input.at<this.lastAt){this.cache.clear();this.identity=identity;}
  this.lastAt=input.at;
  const strategies=input.strategies.map(v=>{
   let event=this.cache.get(v.strategyId)??null;
   if(event?.sourceResultKey!==v.result.key){this.cache.delete(v.strategyId);event=null;}
   if(!event){event=capture(input,v);if(event){immutable(event);this.cache.set(v.strategyId,event);}}
   return {strategyId:v.strategyId,current:{state:v.state,resultState:v.result.state,reason:v.reason??v.result.reason,key:v.result.key,direction:v.result.direction,level:v.result.level,target:v.result.target,stop:v.result.stop,evaluatedMs:input.at,resultAt:v.result.at,lastClosedBar:v.lastClosedBar?{...v.lastClosedBar}:null},event};
  });
  this.latest=immutable({version:'route-opportunity-source-v1' as const,symbol:input.symbol,publishedMs:input.publishedMs,startedMs:input.startedMs,at:input.at,market:{...input.market},strategies});
 }
 snapshot(symbol:string,now:number):Readonly<RouteOpportunitySource>|undefined {
  return clock(now)&&this.latest?.symbol===symbol&&this.latest.at<=now&&now-this.latest.at<=3000?this.latest:undefined;
 }
}
