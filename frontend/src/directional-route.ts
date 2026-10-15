import type {LiveState} from './live';
import type {MarketEvidence} from './market-evidence';
import type {RouteResult} from './route-engine';
import type {RouteStructure} from './route-level-context';
import type {AtrRead} from './intraday-atr';
import {qqqConfirmation} from './qqq-confirmation';
import {rangeFresh} from './daily-range';
import {validBlockPeriod} from './block-memory';
export type RouteSupport='aligned'|'opposed'|'neutral'|'pending';
export interface DirectionalRoute {version:'directional-route-v1';symbol:string;at:number;state:'waiting'|'conditional'|'supported'|'opposed'|'reference-test'|'reached'|'invalidated';side:'CALL'|'PUT'|'PENDIENTE';label:string;anchor:number|null;target:number|null;stop:number|null;distance:number|null;atr5:number|null;atrDaily:number|null;expectedMove:number|null;checks:{price:RouteSupport;flow:RouteSupport;cvd:RouteSupport;levels:RouteSupport;components:RouteSupport;structure:RouteSupport;blocks:RouteSupport};detail:string;invalidation:string;probability:null}
const positive=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n)&&n>0;
const finite=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n);
const etFormatter=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
const date=(n:number)=>etFormatter.format(n);
const fresh=(at:unknown,now:number,limit=3000)=>finite(at)&&at<=now&&now-at<=limit;
/** Read-only synthesis. Uses one canonical price sequence; overlapping indicators never become independent votes. */
export function directionalRoute(s:LiveState|undefined,symbol:string,active:boolean,market:MarketEvidence,route:RouteResult,structure:RouteStructure,now:number,atr?:AtrRead):DirectionalRoute {
 const out:DirectionalRoute={version:'directional-route-v1',symbol,at:now,state:'waiting',side:'PENDIENTE',label:'Dirección pendiente',anchor:null,target:null,stop:null,distance:null,atr5:null,atrDaily:null,expectedMove:null,checks:{price:'pending',flow:'pending',cvd:'pending',levels:'pending',components:'pending',structure:'pending',blocks:'pending'},detail:'Esperando precio vigente, prueba del nivel y referencias propias del día.',invalidation:'Pendiente de una prueba de precio.',probability:null};
 if(!finite(now)||!active||s?.symbol!==symbol||!fresh(s.publishedMs,now)||s.stocks?.phase!=='streaming'||s.metrics?.phase!=='Sesión RTH'||s.metrics.session!==date(now)||market.symbol!==symbol||!fresh(market.at,now)||!market.price.ready||!positive(market.price.spot))return out;
 if(!fresh(route.at,now,90000))return out;
 if(route.state==='invalidated'||route.state==='reached'){out.state=route.state;out.label=route.state==='reached'?'Referencia alcanzada':'Escenario invalidado';out.detail=route.reason;return out;}
 if(!['recovery','test','sustained'].includes(route.state)||![1,-1].includes(route.direction)||!positive(route.level)||!positive(route.target)||!positive(route.stop))return out;
 const sign=route.direction,spot=market.price.spot;
 if((route.target-route.level)*sign<=0||(route.level-route.stop)*sign<=0||!market.levels.some(l=>l.price===route.level&&l.expiration===date(now))||!market.levels.some(l=>l.price===route.target&&l.expiration===date(now)))return out;
 out.side=sign===1?'CALL':'PUT';out.anchor=route.level;out.target=route.target;out.stop=route.stop;out.distance=Math.max(0,(route.target-spot)*sign);
 const pressure=(p:MarketEvidence['flow']):RouteSupport=>!fresh(p.at,now)||!finite(p.coverage)||p.coverage<70||p.coverage>100||!finite(p.signal)||Math.abs(p.signal)>1?'pending':p.signal===0?'neutral':p.signal*sign>0?'aligned':'opposed';
 out.checks.price=market.price.direction===-sign||market.direction===-sign?'opposed':market.price.direction===sign?'aligned':'neutral';out.checks.flow=pressure(market.flow);out.checks.cvd=pressure(market.cvd);
 out.checks.levels=structure.quality==='supported'?'aligned':'pending';
 const basket=qqqConfirmation(s,symbol,true,out.side,now);out.checks.components=basket?.status==='confirms'?'aligned':basket?.status==='contradicts'?'opposed':basket?.status==='mixed'?'neutral':'pending';
 const swing=s.swingReport;
 if(swing?.mode==='observation-only'&&swing.context.version==='structural-swing-v1'&&swing.context.symbol===symbol&&fresh(swing.context.at,now,5000)&&['observing','divergence'].includes(swing.state)&&['CALL','PUT'].includes(swing.context.side))out.checks.structure=swing.context.side===out.side?'aligned':'opposed';
 const block=s.offExchange?.analytics?.blocks,period=block?.regular;
 if(block?.symbol===symbol&&block.date===date(now)&&validBlockPeriod(period)&&period.complete&&period.through_ms<=now&&now-period.through_ms<=300000&&period.count>0){const delta=period.buy-period.sell;out.checks.blocks=Math.abs(delta)<=period.unknown?'neutral':delta*sign>0?'aligned':'opposed';}
 if(atr&&positive(atr.value)&&fresh(atr.endMs,now,300000)&&date(atr.endMs!)===date(now))out.atr5=out.distance/atr.value;
 if(atr?.daily?.symbol===symbol&&positive(atr.daily.value)&&/^\d{4}-\d{2}-\d{2}$/.test(atr.daily.through)&&atr.daily.through===s.chartHistory?.sessions.filter(v=>v.closeMs<now).at(-1)?.date)out.atrDaily=out.distance/atr.daily.value;
 const snap=s.optionSnapshotStatus,range=snap?.dailyRange;
 if(snap?.phase==='ready'&&rangeFresh(snap,now)&&range?.ready&&positive(range.expectedMoveDollars))out.expectedMove=range.expectedMoveDollars;
 const opposed=[out.checks.price,out.checks.flow,out.checks.cvd,out.checks.components,out.checks.structure].includes('opposed')||(spot-route.stop)*sign<=0;
 const supported=route.state==='sustained'&&[out.checks.price,out.checks.flow,out.checks.cvd,out.checks.levels].every(v=>v==='aligned')&&(symbol!=='QQQ'||out.checks.components==='aligned');
 out.state=opposed?'opposed':(route.target-spot)*sign<=0?'reference-test':supported?'supported':'conditional';
 out.label=out.state==='opposed'?`${out.side} · divergencia`:out.state==='reference-test'?'Referencia en prueba':out.state==='supported'?`${out.side} · con apoyo`:`${out.side} · condicionado`;
 const display:Record<RouteSupport,string>={aligned:'acompaña',opposed:'en contra',neutral:'mixto',pending:'pendiente'};
 out.detail=`Precio ${display[out.checks.price]} · flujo ${display[out.checks.flow]} · CVD ${display[out.checks.cvd]} · niveles ${display[out.checks.levels]}${symbol==='QQQ'?' · componentes '+display[out.checks.components]:''} · estructura swing ${display[out.checks.structure]} · bloques ${display[out.checks.blocks]}. Bloques y estructura son contexto separado; no suman porcentajes. Distancias condicionales; ATR e IV no son límites ni indican dirección.`;
 out.invalidation=`Cierre ${sign===1?'bajo':'sobre'} $${route.stop.toFixed(2)} invalida la prueba. Falta de datos suspende la confirmación; no cambia el nivel.`;
 return out;
}



