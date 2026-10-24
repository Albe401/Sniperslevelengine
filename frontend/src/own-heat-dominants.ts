import type {Horizon} from './live';
import type {Group} from './two-maps';

/** Own gamma proxy, never a dealer-position estimate or a Gexbot reading.
 * Core exposure uses gamma * OI * verified multiplier * spot² * .01,
 * with calls positive and puts negative. Executed heat is kept separate.
 */
export interface OwnHeatDominants {
 rows:{strike:number;gex:number;heatContracts:number|null}[];
 positive:{strike:number;gex:number;heatContracts:number|null}|null;
 negative:{strike:number;gex:number;heatContracts:number|null}|null;
 observedMs:number;
 expiry:string;
 complete:boolean;
 gammaUsed:number;
 eligible:number;
}
export function ownHeatDominants(horizons:Horizon[]|undefined,expiry:string,groups:Group[],asof:number):OwnHeatDominants|null {
 if(!expiry||!Number.isSafeInteger(asof)||asof<=0)return null;
 const h=horizons?.find(h=>h.expirations.length===1&&h.expirations[0]===expiry&&h.exposure?.gammaUsed>0);
 if(!h||!Number.isSafeInteger(h.observedMs)||h.observedMs>asof||asof-h.observedMs>300_000||!Array.isArray(h.exposure.strikes))return null;
 const heat=new Map<number,number>();
 const end=Math.floor(asof/60000)*60000;
 // Raw one-minute executions only. routeHeatCells already makes five-minute bands;
 // summing those here would count the same execution up to five times.
 for(const g of groups)if(g.startMs>=end-300_000&&g.startMs<end&&Number.isSafeInteger(g.contracts)&&g.contracts>=0)
  heat.set(g.strike,(heat.get(g.strike)??0)+g.contracts);
 const rows=h.exposure.strikes.filter(r=>Number.isFinite(r.strike)&&r.strike>0&&Number.isFinite(r.netGex))
  .map(r=>({strike:r.strike,gex:r.netGex,heatContracts:groups.length?heat.get(r.strike)??0:null}));
 if(!rows.length||rows.length!==h.exposure.strikes.length||new Set(rows.map(r=>r.strike)).size!==rows.length)return null;
 const positive=rows.filter(r=>r.gex>0).sort((a,b)=>b.gex-a.gex||a.strike-b.strike)[0]??null;
 const negative=rows.filter(r=>r.gex<0).sort((a,b)=>a.gex-b.gex||a.strike-b.strike)[0]??null;
 return {rows,positive,negative,observedMs:h.observedMs,expiry,complete:h.exposure.gexComplete&&h.structure?.complete===true,gammaUsed:h.exposure.gammaUsed,eligible:h.exposure.eligible};
}
