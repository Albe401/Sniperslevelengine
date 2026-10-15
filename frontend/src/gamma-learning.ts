import {invoke,isTauri} from './snapshot-observation';
import type {LiveState} from './live';
import {gammaObservation,GammaSampling} from './gamma-reading';
import {routeChartReading} from './route-live';
import {probabilityEndpoint,routeProbabilityCollector} from './route-probability';
const sampling=new GammaSampling();
let pending:(ReturnType<typeof gammaObservation>&{route_probability?:unknown})|undefined,running=false;
export function collectGammaLearning(s:LiveState) {
 const p=gammaObservation(s);
 if(!isTauri()||!sampling.shouldRecord(p))return;
 pending={...p,route_probability:{endpoint:probabilityEndpoint,reading:routeChartReading(s.symbol,p.capturedMs)?.probability,sample:routeProbabilityCollector.latest(s.symbol,p.capturedMs)}};void drain();
}
async function drain(){
 if(running)return;running=true;
 try{while(pending){const payload=pending;pending=undefined;
  try{await invoke('record_gamma_learning',{payload});}catch(e){console.error('GAMMA_LEARNING_STORAGE_UNAVAILABLE',String(e));}
 }}finally{running=false;}
}
