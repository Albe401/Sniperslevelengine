import type {LiveState} from './live';
import {buildIntradayControl,ControlTracker} from './intraday-control';
import {GammaMovementTracker} from './gamma-movement';
import {gammaLiveModel} from './gamma-live-model';
const control=new ControlTracker(),movement=new GammaMovementTracker();
export function gammaReading(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now()) {
 const reading=control.update(buildIntradayControl(s,symbol,active,now),now);
 return {model:gammaLiveModel(s,symbol,active,now),reading,movement:movement.update(reading,now)};
}
export function gammaObservation(s:LiveState,now=Date.now()) {
 const r=gammaReading(s,s.symbol,true,now);
 return {dealerPositioning:r.reading.dealerPositioning?{...r.reading.dealerPositioning,rows:r.reading.dealerPositioning.rows.filter((x,i)=>i<8||x.roles.length>0)}:undefined,schema:'gamma-map-v1',symbol:s.symbol,capturedMs:now,snapshotMs:r.model.h?.observedMs??0,
  complete:r.model.complete&&r.model.rows.length>0,movement:r.movement,levels:r.model.levels,
  barriers:{upper:r.model.upper.map(x=>({strike:x.strike,force:x.score})),lower:r.model.lower.map(x=>({strike:x.strike,force:x.score}))},
  pulls:{call:r.model.call,put:r.model.put},price:r.reading.priceNow,flow:r.reading.flowControl};
}
export class GammaSampling {
 private key='';private at=0;
 shouldRecord(p:ReturnType<typeof gammaObservation>) {
  const key=JSON.stringify([p.symbol,p.complete,p.movement.kind,p.movement.direction,p.levels.map(l=>[l.id,l.strike,l.dominant])]);
  if(key===this.key&&p.capturedMs>=this.at&&p.capturedMs-this.at<30000)return false;
  this.key=key;this.at=p.capturedMs;return true;
 }
}


