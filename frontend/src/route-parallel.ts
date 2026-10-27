import type {MarketEvidence} from './market-evidence';
import type {RouteResult} from './route-engine';
export type RouteSide='CALL'|'PUT';
const selected=new Map<string,{side:RouteSide;horizon:'session'|'swing2'}>();
export function routeSelection(symbol:string){return selected.get(symbol)??{side:'CALL' as RouteSide,horizon:'swing2' as const};}
export function bindParallelRoute(refresh:()=>void){
 document.querySelectorAll<HTMLButtonElement>('[data-route-side],[data-route-horizon]').forEach(b=>b.onclick=()=>{
  const symbol=b.dataset.routeSymbol;if(!symbol)return;const prior=routeSelection(symbol);
  const side=b.dataset.routeSide,horizon=b.dataset.routeHorizon;
  if(side==='CALL'||side==='PUT')selected.set(symbol,{...prior,side});
  if(horizon==='session'||horizon==='swing2')selected.set(symbol,{...prior,horizon});
  refresh();
 });
}
/** Alternative conditions from the same current map; never two simultaneous confirmed signals. */
export function parallelScenarios(market:MarketEvidence,result:RouteResult,now:number){
 return ([1,-1] as const).map(direction=>{
  const side:RouteSide=direction===1?'CALL':'PUT',spot=market.price.spot;
  const ready=Number.isFinite(now)&&Number.isFinite(market.at)&&market.price.ready&&typeof spot==='number'&&Number.isFinite(spot)&&spot>0&&market.at<=now&&now-market.at<=3000;
  // The source evaluates closed one-minute bars. A tick cannot renew, resolve or move its frozen geometry.
  const actual=ready&&result.direction===direction&&Number.isFinite(result.at)&&result.at<=now&&now-result.at<=90000&&typeof result.level==='number'&&Number.isFinite(result.level)&&result.level>0&&(result.deadlineMs===undefined||(Number.isFinite(result.deadlineMs)&&result.deadlineMs>=now))&&['recovery','test','sustained'].includes(result.state);
  const levels=ready?market.levels.filter(l=>Number.isFinite(l.price)&&l.price>0&&l.observed<=now&&(l.asof??l.observed)<=now&&now-(l.asof??l.observed)<=900000):[];
  const ahead=levels.filter(l=>(l.price-spot!)*direction>spot!*.0003).sort((a,b)=>Math.abs(a.price-spot!)-Math.abs(b.price-spot!));
  const barrier=ahead.find(l=>l.role!=='pull'&&l.role!=='vacuum-edge');
  const trigger=actual?result.level:barrier?.price??null;
  const after=trigger==null?[]:ahead.filter(l=>(l.price-trigger)*direction>spot!*.0003);
  const references=[...new Set(after.map(l=>l.price))];
  const validTarget=actual&&result.target!=null&&Number.isFinite(result.target)&&result.target>0;
  const target=validTarget?result.target:references[0]??null;
  const next=references.find(p=>target!=null&&(p-target)*direction>0)??null;
  const stop=actual&&result.stop!=null&&Number.isFinite(result.stop)&&result.stop>0&&(result.level!-result.stop)*direction>0?result.stop:null;
  return {side,direction,trigger,target,next,stop,actual,provisional:!actual&&!!barrier?.provisional,
   state:!ready?'Esperando datos vigentes':actual?result.state==='sustained'?'Continuación en observación':result.state==='test'?'Prueba · falta continuación':'Recuperación candidata':trigger==null?'Sin nivel de activación':'Alternativa condicionada'};
 });
}
