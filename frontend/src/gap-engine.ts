import type {LiveState} from './live';
import type {MarketEvidence,PressureEvidence} from './market-evidence';
import type {AtrRead} from './intraday-atr';
import {rangeFresh} from './daily-range';
export interface PriceGap {id:string;kind:'OPENING'|'SESSION_RANGE';born:string;origin:'UP'|'DOWN';lower:number;upper:number;state:'OPEN'|'PARTIAL'|'FILLED'|'RECLAIMED';fillPct:number;remainingLower:number;remainingUpper:number;ageSessions:number;filledAt:string|null;reclaimedAt:string|null;verified:boolean;exDividend:boolean}
export interface GapReport {version:string;symbol:string;phase:string;through:string;at:number;coveragePct:number;actionsVerified:boolean;gaps:PriceGap[];detail:string}
export interface GapCandidate {gap:PriceGap;side:'above'|'below'|'inside';distancePct:number;distanceAtr:number|null;entry:number;fillTarget:number;expectedBand:'inside'|'partial'|'outside'|'pending';path:'barrier'|'unverified';attractionScore:number|null;support:'aligned'|'opposed'|'mixed'|'pending';reasons:string[]}
export interface GapReading {version:'gap-reading-v1';symbol:string;at:number;state:'ready'|'partial'|'pending';above:GapCandidate[];below:GapCandidate[];inside:GapCandidate[];detail:string;probability:null}
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v);
const positive=(v:unknown):v is number=>finite(v)&&v>0;
const recent=(at:unknown,now:number,limit:number)=>finite(at)&&at<=now&&now-at<=limit;
const dayFormat=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
export function gapReading(s:LiveState|undefined,symbol:string,active:boolean,market:MarketEvidence|undefined,atr:AtrRead,now=Date.now()):GapReading {
 const out:GapReading={version:'gap-reading-v1',symbol,at:now,state:'pending',above:[],below:[],inside:[],detail:'Esperando historial de gaps y precio propio vigente. Sin probabilidad de relleno.',probability:null};
 const r=s?.gapReport,tick=s?.ticks?.[symbol];
 if(!finite(now)||!active||s?.symbol!==symbol||s.stocks?.phase!=='streaming'||!recent(s.publishedMs,now,3000)||!r||r.version!=='price-gap-v1'||r.symbol!==symbol||!['ready','partial'].includes(r.phase)||!recent(r.at,now,5000)||!Array.isArray(r.gaps)||r.gaps.length>122||tick?.symbol!==symbol||!positive(tick.price)||!recent(tick.tradeMs,now,15000))return out;
 const today=dayFormat.format(now),last=s.chartHistory?.sessions.filter(v=>v.closeMs<now).at(-1)?.date;
 if(r.through!==last)return out;
 if(!finite(r.coveragePct)||r.coveragePct<0||r.coveragePct>100)return out;
 const spot=tick.price,a=atr.daily?.symbol===symbol&&atr.daily.through===last&&positive(atr.daily.value)?atr.daily.value:null;
 const own=market?.symbol===symbol&&market.session===today&&recent(market.at,now,3000);
 const pressure=(p:PressureEvidence|undefined)=>own&&p&&recent(p.at,now,3000)&&finite(p.signal)&&Math.abs(p.signal)<=1&&finite(p.coverage)&&p.coverage>=70&&p.coverage<=100?p.signal:null;
 const flow=pressure(market?.flow),cvd=pressure(market?.cvd);
 const snap=s.optionSnapshotStatus,range=snap?.phase==='ready'&&rangeFresh(snap,now)&&snap.dailyRange?.ready?snap.dailyRange:undefined;
 const validRange=range&&positive(range.snapshotLower)&&positive(range.snapshotUpper)&&range.snapshotLower<range.snapshotUpper&&positive(range.spot)&&positive(range.expectedMoveDollars)&&Math.abs(range.snapshotLower-(range.spot-range.expectedMoveDollars))<.01&&Math.abs(range.snapshotUpper-(range.spot+range.expectedMoveDollars))<.01;
 for(const g of r.gaps){if(!['OPEN','PARTIAL'].includes(g.state)||!['OPENING','SESSION_RANGE'].includes(g.kind)||!['UP','DOWN'].includes(g.origin)||!positive(g.lower)||!positive(g.upper)||g.lower>=g.upper||!positive(g.remainingLower)||!positive(g.remainingUpper)||g.remainingLower<g.lower||g.remainingUpper>g.upper||g.remainingLower>=g.remainingUpper||!finite(g.fillPct)||g.fillPct<0||g.fillPct>=100||!Number.isInteger(g.ageSessions)||g.ageSessions<0||!/^\d{4}-\d{2}-\d{2}$/.test(g.born)||g.born>today)continue;
  if(Math.abs((g.remainingUpper-g.remainingLower)-(g.upper-g.lower)*(1-g.fillPct/100))>Math.max(1e-8,(g.upper-g.lower)*1e-8)||(g.state==='OPEN'&&g.fillPct!==0)||(g.state==='PARTIAL'&&g.fillPct<=0))continue;
  const side=spot<g.remainingLower?'above':spot>g.remainingUpper?'below':'inside',sign=side==='above'?1:side==='below'?-1:g.origin==='UP'?-1:1;
  const entry=side==='above'?g.remainingLower:side==='below'?g.remainingUpper:spot,fillTarget=sign===1?g.remainingUpper:g.remainingLower,distance=Math.abs(entry-spot);
  const support=flow==null||cvd==null?'pending':flow*sign>0&&cvd*sign>0?'aligned':flow*sign<0||cvd*sign<0?'opposed':'mixed';
  const expectedBand:GapCandidate['expectedBand']=!validRange?'pending':g.remainingLower>=range!.snapshotLower!&&g.remainingUpper<=range!.snapshotUpper!?'inside':g.remainingUpper<range!.snapshotLower!||g.remainingLower>range!.snapshotUpper!?'outside':'partial';
  const levels=own?market!.levels.filter(l=>positive(l.price)&&recent(l.asof??l.observed,now,900000)&&l.expiration===today):[];
  const barrier=levels.some(l=>(l.price-spot)*sign>0&&(fillTarget-l.price)*sign>=0&&['barrier','call-wall','put-wall'].includes(l.role??'')&&finite(l.strength)&&l.strength>=60);
  const magnet=levels.some(l=>l.role==='magnet'&&(l.price-fillTarget)*sign>=0&&Math.abs(l.price-fillTarget)/spot<=.005);
  const reasons=[`Estado ${g.state}; relleno observado ${g.fillPct.toFixed(1)}%`,`${expectedBand==='pending'?'Banda esperada pendiente':`Banda del snapshot: ${expectedBand}`} (no recorrido restante)`,barrier?'Barrera importante en el camino':'Cobertura del camino no demuestra ausencia de barreras'];
  const valid=g.verified&&r.actionsVerified&&!g.exDividend;
  if(!valid)reasons.push(g.exDividend?'Fecha exdividendo: excluido de atracción':'Historial/acciones corporativas incompletos: atracción pendiente');
  let score=valid?Math.round(Math.max(0,Math.min(100,(a==null?15:30*Math.max(0,1-distance/a))+ (expectedBand==='inside'?15:expectedBand==='partial'?5:0)+(support==='aligned'?20:support==='opposed'?-25:0)+(own&&market!.price.direction===sign?10:0)+(magnet?10:0)-(barrier?25:0)-Math.min(5,g.ageSessions/6)))):null;
  if(score!=null&&expectedBand==='outside')score=Math.min(score,30);
  if(score!=null&&(a==null||support==='pending'||expectedBand==='pending'))score=Math.min(score,40);
  const candidate:GapCandidate={gap:{...g},side,distancePct:100*distance/spot,distanceAtr:a==null?null:distance/a,entry,fillTarget,expectedBand,path:barrier?'barrier':'unverified',attractionScore:score,support,reasons};
  (side==='above'?out.above:side==='below'?out.below:out.inside).push(candidate);
 }
 const nearest=(rows:GapCandidate[])=>rows.sort((a,b)=>a.distancePct-b.distancePct||a.gap.id.localeCompare(b.gap.id)).filter((v,i,all)=>!all.slice(0,i).some(p=>Math.min(p.gap.remainingUpper,v.gap.remainingUpper)>Math.max(p.gap.remainingLower,v.gap.remainingLower))).slice(0,3);
 out.above=nearest(out.above);out.below=nearest(out.below);out.inside=nearest(out.inside);out.state=r.phase==='ready'?'ready':'partial';out.detail=r.detail+' Score heurístico de confluencia, no probabilidad. Gaps superpuestos no se suman como votos. Sin integración de órdenes ni FVG.';return out;
}
