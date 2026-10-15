import type {Candle} from './model';
import type {LiveState} from './live';
import type {MarketEvidence} from './market-evidence';
import type {RouteResult} from './route-engine';
import {intradayAtr} from './intraday-atr';
export const probabilityEndpoint='target-close-before-route-invalidation-10m-v1';
export const probabilityVersion='quick-route-joint-v1';
export const probabilityHorizon=600000;
export interface ProbabilitySample {
 id:string;version:string;endpoint:string;source:'live';symbol:string;session:string;connection:number;
 at:number;deadline:number;direction:number;price:number;target:number;stop:number;lastBar:number;
 features:Record<string,number|null>;observedConfluence?:{call:number;put:number};outcome:'pending'|'hit'|'invalidated'|'timeout'|'missing';resolvedAt:number|null;reason:string;
}
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v);
const dateFormatter=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
const date=(n:number)=>dateFormatter.format(n);
const storageKey='snipers.route-probability.v1';
/** Freeze one joint feature vector at availability. Missing inputs remain null, not votes. */
export function probabilityFeatures(s:LiveState,e:MarketEvidence,r:RouteResult,now:number):Record<string,number|null>{
 const spot=e.price.spot!,anchor=e.levels.find(l=>l.price===r.level),a=intradayAtr(s,s.symbol,true,now);
 const metricAt=s.metrics.asofMs,metricFresh=finite(metricAt)&&metricAt<=now&&now-metricAt<=180000;
 const profile=anchor?.evidence,profileFresh=profile&&profile.at<=now&&now-profile.at<=300000;
 const rows=e.levels.filter(l=>l.price>0),ahead=rows.filter(l=>(l.price-spot)*r.direction>0);
 const b=s.offExchange?.analytics?.blocks,p=b?.regular;
 const blocksFresh=b?.symbol===s.symbol&&b.date===date(now)&&p?.complete&&finite(p.through_ms)&&p.through_ms<=now&&now-p.through_ms<=180000;
 const blockTotal=p?p.buy+p.sell+p.unknown:0;
 const blockValid=blocksFresh&&p&&[p.buy,p.sell,p.unknown].every(v=>finite(v)&&v>=0)&&blockTotal>0;
 const values:Record<string,unknown>={
  priceSignal:e.score.components.find(c=>c.name==='Precio y VWAP')?.signal,
  flowSignal:e.flow.signal,flowCoverage:e.flow.coverage,cvdSignal:e.cvd.signal,cvdCoverage:e.cvd.coverage,
  structureSignal:e.score.components.find(c=>c.name==='Walls, imán y barreras')?.signal,
  routeDirection:r.direction,
  targetBps:r.target!=null?(r.target-spot)*r.direction/spot*10000:null,
  riskBps:r.stop!=null?(spot-r.stop)*r.direction/spot*10000:null,
  targetAtr:r.target!=null&&a.value!=null?Math.abs(r.target-spot)/a.value:null,
  targetDailyAtr:r.target!=null&&a.daily?.value?Math.abs(r.target-spot)/a.daily.value:null,
  rvol:metricFresh?s.metrics.rvol:null,anchorStrength:anchor?.strength,
  anchorChange:anchor?.change,barriersAhead:ahead.filter(l=>l.role==='barrier').length,
  provisional:rows.some(l=>l.provisional)?1:0,
  oiShare:profileFresh?profile.oiShare:null,gammaShare:profileFresh?profile.gammaShare:null,
  volumeOi:profileFresh?profile.volumeOi:null,iv:profileFresh?profile.iv:null,
  vanna:profileFresh?profile.vanna:null,charm:profileFresh?profile.charm:null,
  blockPressure:blockValid?(p.buy-p.sell)/blockTotal:null,
 };
 const snap=s.optionSnapshotStatus,today=date(now),universe=[...new Set((snap?.expirations??[]).filter(d=>d>=today))].sort();
 const friday=new Date(today+'T12:00:00Z');friday.setUTCDate(friday.getUTCDate()+(5-friday.getUTCDay()+7)%7);
 for(const mode of ['DAY','WEEK','NEXT4']){
  const hs=snap?.horizons?.filter(h=>h.mode===mode)??[],h=hs.length===1?hs[0]:undefined,x=h?.exposure;
  const expected=mode==='DAY'?[today]:mode==='WEEK'?universe.filter(d=>d<=friday.toISOString().slice(0,10)):universe.slice(0,4);
  const shape=!!h&&h.expirations.length>0&&h.expirations.every(d=>/^\d{4}-\d{2}-\d{2}$/.test(d)&&d>=today)&&h.expirations.join('|')===expected.join('|')&&(mode!=='NEXT4'||expected.length===4);
  const gross=x&&finite(x.callDex)&&finite(x.putDex)?Math.abs(x.callDex)+Math.abs(x.putDex):0;
  const valid=shape&&snap?.phase==='ready'&&h!.observedMs===snap.observedMs&&h!.observedMs<=now&&now-h!.observedMs<=300000&&date(h!.observedMs)===today&&x?.dexComplete&&x.eligible>0&&x.deltaUsed===x.eligible&&x.unknownOi===0&&gross>0&&finite(x.callDexPct)&&finite(x.putDexPct)&&x.callDexPct>=0&&x.putDexPct>=0&&Math.abs(x.callDexPct+x.putDexPct-100)<.01&&Math.abs(x.callDexPct-100*Math.abs(x.callDex!)/gross)<.01&&finite(x.netDex)&&Math.abs(x.netDex-x.callDex!-x.putDex!)<Math.max(1e-6,gross*1e-9);
  values[mode.toLowerCase()+'DeltaShare']=valid?x!.callDexPct:null;
 }
 return Object.fromEntries(Object.entries(values).map(([k,v])=>[k,finite(v)?v:null]));
}
/** Outcome collection is independent of admission/broker permissions and never trains in the UI. */
export class RouteProbabilityCollector {
 private rows:ProbabilitySample[]=[];private lastStart=new Map<string,number>();private clock=0;
 error='';
 constructor(private persist=false){
  if(!persist)return;
  try{const saved=JSON.parse(localStorage.getItem(storageKey)??'[]');
   if(Array.isArray(saved))this.rows=saved.filter(validProbabilitySample).slice(-4000).map(row=>row.outcome==='pending'?{...row,outcome:'missing',reason:'Aplicación reiniciada; continuidad no comprobada'}:row);
   for(const row of this.rows)this.lastStart.set(row.symbol,Math.max(this.lastStart.get(row.symbol)??0,row.at));
  }catch{this.error='Historial probabilístico no recuperado';}
 }
 snapshot(){return this.rows.map(r=>({...r,features:{...r.features}}));}
 latest(symbol:string,now:number){const row=this.rows.slice().reverse().find(r=>r.symbol===symbol&&r.at<=now);return row?{...row,features:{...row.features},observedConfluence:row.observedConfluence?{...row.observedConfluence}:undefined}:undefined;}
 current(symbol:string,now:number){const row=this.rows.slice().reverse().find(r=>r.symbol===symbol&&r.outcome==='pending'&&r.at<=now&&now<=r.deadline);return row?{...row,features:{...row.features}}:undefined;}
 private save(){if(!this.persist)return;try{localStorage.setItem(storageKey,JSON.stringify(this.rows));this.error='';}catch{this.error='Registro probabilístico conservado en memoria; almacenamiento no disponible';}}
 observe(s:LiveState|undefined,e:MarketEvidence,r:RouteResult,bars:Candle[],active:boolean,now:number){
  if(!finite(now))return;
  const session=s?.chartHistory?.sessions.find(x=>x.date===date(now)),tick=s?.ticks?.[e.symbol];
  const ready=active&&s?.symbol===e.symbol&&s.stocks?.phase==='streaming'&&s.metrics?.session===date(now)&&s.metrics.phase==='Sesión RTH'&&finite(s.publishedMs)&&s.publishedMs<=now&&now-s.publishedMs<=3000&&tick?.symbol===e.symbol&&finite(tick.tradeMs)&&tick.tradeMs<=now&&now-tick.tradeMs<=15000&&session&&session.openMs<=now&&now<session.closeMs;
  let dirty=false;
  for(const row of this.rows.filter(x=>x.outcome==='pending')){
   const finish=(outcome:ProbabilitySample['outcome'],reason:string,at=now)=>{row.outcome=outcome;row.reason=reason;row.resolvedAt=at;dirty=true;};
   if(now<this.clock||!ready||row.symbol!==s!.symbol||row.session!==date(now)||row.connection!==s!.startedMs){finish('missing','Interrupción, cambio de activo/sesión/conexión o reloj');continue;}
   const subsequent=bars.filter(b=>b.timestampMs>row.lastBar&&b.timestampMs+60000>row.at&&b.timestampMs+60000<=now&&b.timestampMs+60000<=row.deadline).sort((a,b)=>a.timestampMs-b.timestampMs);
   for(const bar of subsequent){
    const first=Math.floor(row.at/60000)*60000;
    const expected=row.lastBar?row.lastBar+60000:first;
    if(bar.timestampMs!==expected||![bar.open,bar.high,bar.low,bar.close,bar.volume].every(finite)||bar.volume<0||bar.low<=0||bar.high<Math.max(bar.open,bar.close)||bar.low>Math.min(bar.open,bar.close)){finish('missing','Hueco o vela inválida: resultado desconocido');break;}
    row.lastBar=bar.timestampMs;dirty=true;
    // The existing route defines attainment/invalidation by closed price, never ambiguous intrabar order.
    if((bar.close-row.stop)*row.direction<0){finish('invalidated','Cierre perdió la invalidación congelada',bar.timestampMs+60000);break;}
    if((bar.close-row.target)*row.direction>=0){finish('hit','Cierre alcanzó la referencia antes de invalidarse',bar.timestampMs+60000);break;}
   }
   if(row.outcome==='pending'&&r.key===row.id&&r.state==='invalidated'&&/secuencia excedió|cierre perdió/i.test(r.reason))finish('invalidated',r.reason,Math.min(now,row.deadline));
   if(row.outcome==='pending'&&(r.key!==row.id||r.state==='invalidated'||r.state==='waiting'))finish('missing','La ruta cambió o dejó de ser comparable antes de completar su evaluación');
   if(row.outcome==='pending'&&now>=row.deadline){
    const finalClose=Math.floor(row.deadline/60000)*60000;
    const complete=row.lastBar+60000>=finalClose;
    finish(complete?'timeout':'missing',complete?'No alcanzó la referencia dentro de 10 min':'Faltan cierres para evaluar los 10 min',complete?row.deadline:now);
   }
  }
  this.clock=now;
  const spot=e.price.spot,dayLevels=e.levels.filter(l=>l.expiration===date(now));
  const previous=this.lastStart.get(e.symbol);
  const pressureFresh=[e.flow,e.cvd].every(p=>finite(p.at)&&p.at<=now&&now-p.at<=3000&&finite(p.signal)&&finite(p.coverage)&&p.coverage>=70);
  if(ready&&r.state==='sustained'&&r.key&&finite(r.at)&&r.at<=now&&now-r.at<=3000&&e.at<=now&&now-e.at<=3000&&e.price.ready&&e.score.call!=null&&pressureFresh&&finite(spot)&&spot>0&&[1,-1].includes(r.direction)&&r.target!=null&&r.stop!=null&&[r.target,r.stop].every(v=>finite(v)&&v>0)&&(r.target-spot)*r.direction>0&&(spot-r.stop)*r.direction>0&&dayLevels.some(l=>l.price===r.target)&&dayLevels.some(l=>l.price===r.level)&&now+probabilityHorizon<session!.closeMs&&(!previous||now-previous>=probabilityHorizon)&&!this.rows.some(x=>x.id===r.key)){
   const features=probabilityFeatures(s!,e,r,now);
   this.rows.push({id:r.key,version:probabilityVersion,endpoint:probabilityEndpoint,source:'live',symbol:e.symbol,session:date(now),connection:s!.startedMs,at:now,deadline:now+probabilityHorizon,direction:r.direction,price:spot,target:r.target,stop:r.stop,lastBar:Math.floor(now/60000)*60000-60000,features,observedConfluence:e.score.put!=null?{call:e.score.call,put:e.score.put}:undefined,outcome:'pending',resolvedAt:null,reason:'Esperando resultado futuro'});
   this.lastStart.set(e.symbol,now);this.rows=this.rows.slice(-4000);dirty=true;
  }
  if(dirty){if(this.persist)while(JSON.stringify(this.rows).length>2000000&&this.rows.length>1)this.rows.shift();this.save();}
 }
}
export function validProbabilitySample(v:unknown):v is ProbabilitySample {
 const r=v as ProbabilitySample;
 return !!r&&r.version===probabilityVersion&&r.endpoint===probabilityEndpoint&&r.source==='live'&&typeof r.id==='string'&&typeof r.symbol==='string'&&typeof r.session==='string'&&[1,-1].includes(r.direction)&&[r.at,r.deadline,r.price,r.target,r.stop,r.lastBar,r.connection].every(finite)&&r.at>0&&r.session===date(r.at)&&r.price>0&&r.target>0&&r.stop>0&&r.deadline-r.at===probabilityHorizon&&(r.target-r.price)*r.direction>0&&(r.price-r.stop)*r.direction>0&&['pending','hit','invalidated','timeout','missing'].includes(r.outcome)&&r.features!=null&&typeof r.features==='object'&&Object.values(r.features).every(v=>v===null||finite(v));
}
export const routeProbabilityCollector=new RouteProbabilityCollector(true);
