import {mapLevelData} from './map-level-data';
import type {LiveState} from './live';
/** Saved references may accompany older candles, explicitly without live scores/signals. */
export function capturedChartLevels(state:LiveState|undefined,symbol:string,capturedMs:number|undefined,now:number){
 if(capturedMs==null||!Number.isFinite(capturedMs)||capturedMs!==now||state?.symbol!==symbol||!Number.isFinite(state.publishedMs)||state.publishedMs>capturedMs)return [];
 return mapLevelData(state,symbol,capturedMs).levels.map(l=>({...l,importance:null,name:l.name+' · snapshot'}));
}
