import type {RouteResult} from './route-engine';
import type {MarketEvidence} from './market-evidence';
import {assessRoute} from './route-assessment';
/** A visual destination only: no fabricated future OHLC/time or trade authorization. */
export function routeChartScenario(r:RouteResult|undefined,spot:number|undefined,now:number,current:boolean){
 if(!Number.isFinite(now)||!current||!r||!Number.isFinite(r.at)||r.state!=='sustained'||![1,-1].includes(r.direction)||spot==null||!Number.isFinite(spot)||spot<=0||r.at>now||now-r.at>90000||r.target==null||r.stop==null||![r.target,r.stop].every(v=>Number.isFinite(v)&&v>0)||(r.target-spot)*r.direction<=0||(spot-r.stop)*r.direction<=0)return undefined;
 return {target:r.target,stop:r.stop,direction:r.direction};
}

/** Route state only; technical BOS/ChoCH marks never substitute for the engine. */
export function routeChartStatus(r:RouteResult|undefined,current:boolean,market?:MarketEvidence,now=Date.now()){
 if(!current||!r)return 'Ruta intradía · pendiente';
 if(market)return 'Ruta intradía · '+assessRoute(r,market,now).label;
 return ({waiting:'Ruta intradía · sin confirmar',range:'Ruta intradía · esperando prueba',recovery:'Ruta intradía · recuperación/rechazo candidato',test:'Ruta intradía · prueba del nivel',sustained:r.direction===1?'Ruta intradía · CALL sostenido ↑':'Ruta intradía · PUT sostenido ↓',invalidated:'Ruta intradía · invalidada',reached:'Ruta intradía · objetivo alcanzado'})[r.state];
}
