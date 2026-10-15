import type {LiveState} from '../live';
import {gammaLiveModel} from '../gamma-live-model';
import {gammaChartData} from '../gamma-chart-data';
export interface PointSeries {label:string;unit:string;observedMs:number;points:{strike:number;value:number;side:'buy'|'sell'}[]}
export interface DualData {
 symbol:string;observedMs:number|null;status:string;partial:boolean;historical:boolean;
 bars:{timestampMs:number;close:number;vwap?:number|null}[];interval:number;
 rows:{strike:number;netGex:number}[];spot:number|null;
 levels:{label:string;price:number;color:string}[];points?:PointSeries;
}
export function dualData(state:LiveState|undefined,symbol:string,active:boolean,interval=2,now=Date.now(),points?:PointSeries):DualData {
 const model=gammaLiveModel(state,symbol,active,now),chart=gammaChartData(state,symbol,active,interval,now);
 const current=chart.currentOverlay&&model.fresh;
 const rows=current?model.profileGexRows.map(r=>({strike:r.strike,netGex:r.net_gex})):[];
 const spot=current&&model.h?.exposure.spot!=null?model.h.exposure.spot:null;
 const levels=current?model.levels.filter(l=>l.strike!=null).map(l=>({label:l.label,price:l.strike!,color:l.id==='call'?'#26f46b':l.id==='put'?'#ff4958':'#23d9e6'})):[];
 if(current&&model.flip!=null)levels.push({label:'ZERO GAMMA',price:model.flip,color:'#eac348'});
 const usablePoints=current&&points&&points.label.trim()&&points.unit.trim()&&points.observedMs===model.h?.observedMs&&points.points.every(p=>Number.isFinite(p.strike)&&p.strike>0&&Number.isFinite(p.value)&&(p.side==='buy'||p.side==='sell'))?points:undefined;
 return {symbol,interval,observedMs:model.h?.observedMs??null,status:chart.historical?'PRECIO HISTÓRICO · GEX ACTUAL OCULTO':model.status,partial:!model.complete,historical:chart.historical,spot,bars:chart.bars.map((b,i)=>({timestampMs:b.timestampMs,close:b.close,vwap:chart.vwap[i]?.value})),rows,levels,points:usablePoints};
}
