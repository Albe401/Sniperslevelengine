import type {LiveState} from './live';
import type {PriceNow} from './price-now';
import type {Candle} from './model';
import {interpretationFresh} from './confirmations';
import {currentMapHorizon} from './reading-display';
import {currentSessionCandles} from './candle-series';

export interface MapLevel {name:string;level:number;score?:number;provisional:boolean}
export interface LevelEvent {name:string;level:number;direction:number;action:string;at:number;sustained:boolean}
export interface IntradayMap {levels:MapLevel[];event?:LevelEvent;quality:string;complete:boolean;at?:number;comparisonKey:string;events:LevelEvent[]}
const positive=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n)&&n>0;
const dateET=(n:number)=>new Date(n).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
/** Describe an actual interaction with an existing level, never a wall's presumed intent. */
export function observeMappedLevel(l:MapLevel,bars:Candle[],spot:number):LevelEvent|undefined {
 const n=l.level,tolerance=spot*.0003;
 for(let i=bars.length-2;i>=Math.max(1,bars.length-5);i--){
  const b=bars[i],previous=bars[i-1],after=bars.slice(i+1);
  const above=spot>n+tolerance&&after.every(x=>x.close>n+tolerance);
  const below=spot<n-tolerance&&after.every(x=>x.close<n-tolerance);
  const touched=b.low<=n+tolerance&&b.high>=n-tolerance;
  const brokeUp=previous.close<n-tolerance&&b.close>n+tolerance;
  const brokeDown=previous.close>n+tolerance&&b.close<n-tolerance;
  if(above&&(touched||brokeUp))return {name:l.name,level:n,direction:1,action:brokeUp?'Recuperación sostenida':'Defensa observada',at:b.timestampMs,sustained:after.length>=2};
  if(below&&(touched||brokeDown))return {name:l.name,level:n,direction:-1,action:brokeDown?'Pérdida sostenida':'Rechazo observado',at:b.timestampMs,sustained:after.length>=2};
 }
 return undefined;
}

export function readIntradayMap(s:LiveState|undefined,symbol:string,active:boolean,p:PriceNow,now=Date.now()):IntradayMap {
 const out:IntradayMap={levels:[],events:[],quality:'Estructura: pendiente',complete:false,comparisonKey:''};
 if(!active||s?.symbol!==symbol||!interpretationFresh(s.publishedMs,now))return out;
 const snap=s.optionSnapshotStatus;
 const h=currentMapHorizon(snap,now);
 if(!h||h.mode!=='DAY')return out;
 const partial=h.structure.provisional;
 const st=partial??(h.structure.complete?h.structure:undefined);
 if(!st)return out;
 out.complete=!partial&&h.structure.complete;
 out.quality=partial?`Estructura: provisional · ${partial.coveredContracts}/${partial.totalContracts} contratos utilizables`:'Estructura: completa y vigente';
 out.at=snap.observedMs??undefined;
 const add=(name:string,level:unknown,score?:number)=>{if(positive(level))out.levels.push({name,level,score:score!=null&&Number.isFinite(score)&&score>=0&&score<=100?score:undefined,provisional:!out.complete});};
 add('PUT WALL',st.putWall?.strike,st.putWall?.score);add('CALL WALL',st.callWall?.strike,st.callWall?.score);add('IMÁN PRINCIPAL',st.selected.M1);
 for(const route of st.routes)if(partial||route.structuralReady){add(`BARRERA ${route.direction}`,route.barrier);add(`OBJETIVO ${route.direction}`,route.target);}
 // Provisional membership is not exposed by the provider snapshot model: never compare
 // scores across partial universes just because their contract counts happen to match.
 out.comparisonKey=out.complete?JSON.stringify([symbol,dateET(now),s.startedMs,h.expirations,h.referenceSource,h.structure.rows,out.levels.map(l=>[l.name,l.level])]):'';
 if(!p.ready||!positive(p.spot))return out;
 let bars=currentSessionCandles(s.chartHistory,1,now).filter(b=>b.complete&&b.sessionDate===dateET(now)).slice(-7);
 for(let i=bars.length-1;i>0;i--)if(bars[i].timestampMs-bars[i-1].timestampMs!==60000){bars=bars.slice(i);break;}
 if(!bars.length||now-bars.at(-1)!.timestampMs>150000)return out;
 out.events=out.levels.map(l=>observeMappedLevel(l,bars,p.spot!)).filter((e):e is LevelEvent=>!!e).sort((a,b)=>b.at-a.at||Math.abs(a.level-p.spot!)-Math.abs(b.level-p.spot!));
 out.event=out.events[0];
 return out;
}

/** Changes are computed from two fresh complete snapshots of the same mapped universe. */
export class MapWeightTracker {
 private previous?:{key:string;at:number;levels:MapLevel[]};private change='';
 update(m:IntradayMap|undefined):string {
  if(!m?.complete||!m.comparisonKey||m.at==null){this.previous=undefined;this.change='';return m?.levels.length?'Peso provisional · cambio no verificable':'Esperando niveles vigentes';}
  const previous=this.previous;
  if(!previous||previous.key!==m.comparisonKey||m.at<previous.at||m.at-previous.at>900000){this.previous={key:m.comparisonKey,at:m.at,levels:m.levels.map(l=>({...l}))};this.change='Esperando otra actualización comparable';return this.change;}
  if(m.at===previous.at)return this.change;
  const changes=m.levels.filter(l=>/^(CALL|PUT) WALL$/.test(l.name)).flatMap(l=>{const before=previous.levels.find(x=>x.name===l.name&&x.level===l.level);if(l.score==null||before?.score==null)return [];const delta=l.score-before.score;return Math.abs(delta)>=5?[{l,delta}]:[];}).sort((a,b)=>Math.abs(b.delta)-Math.abs(a.delta));
  const best=changes[0];
  this.change=best?`${best.l.name} $${best.l.level.toFixed(2)} ${best.delta>0?'gana':'pierde'} peso · ${best.delta>0?'+':''}${best.delta.toFixed(1)} puntos`:'Sin cambio relevante de peso entre muros';
  this.previous={key:m.comparisonKey,at:m.at,levels:m.levels.map(l=>({...l}))};
  return this.change;
 }
}
