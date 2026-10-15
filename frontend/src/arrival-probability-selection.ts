import type {ArrivalHorizon} from './arrival-probability-engine';
export interface ArrivalReading {side:number;probability:number|null;horizon:ArrivalHorizon;target:number;at:number}
/** Compare the same endpoint horizon only. Unknown probabilities are not zero. */
export function highestArrivalProbability(readings:ArrivalReading[],now:number,horizon:ArrivalHorizon='session'){
 if(!Number.isFinite(now))return undefined;
 const candidates=readings.filter(r=>r.horizon===horizon);
 if(!candidates.length||candidates.some(r=>![1,-1].includes(r.side)||!Number.isFinite(r.at)||r.at>now||now-r.at>3000||!Number.isFinite(r.target)||r.target<=0||r.probability==null||!Number.isFinite(r.probability)||r.probability<0||r.probability>1)||new Set(candidates.map(r=>r.side)).size!==candidates.length)return undefined;
 const sorted=candidates.slice().sort((a,b)=>b.probability!-a.probability!);
 if(sorted[1]&&Math.abs(sorted[0].probability!-sorted[1].probability!)<=1e-12)return undefined;
 return {...sorted[0]};
}
