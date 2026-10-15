import type {Horizon,OffSummary} from './live';
import {rangeFresh,type RangeSnapshot} from './daily-range';

// Display selection only: intraday control continues to require the DAY universe.
export function mapHorizon(horizons:Horizon[]=[]):Horizon|undefined {
 return horizons.find(h=>h.mode==='DAY')??horizons.find(h=>h.mode==='NEXT_SESSION');
}
/** A reference and its wall score must belong to one current snapshot and expiration. */
export function currentMapHorizon(snapshot:(RangeSnapshot&{horizons:Horizon[]})|undefined,now:number):Horizon|undefined {
 if(snapshot?.phase!=='ready'||!rangeFresh(snapshot,now))return;
 const h=mapHorizon(snapshot.horizons);
 if(!h||snapshot.horizons.filter(x=>x.mode===h.mode).length!==1||h.observedMs!==snapshot.observedMs||!rangeFresh({phase:'ready',observedMs:h.observedMs},now)||h.expirations?.length!==1)return;
 const today=new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'}),expiration=h.expirations[0];
 if(!/^\d{4}-\d{2}-\d{2}$/.test(expiration)||(h.mode==='DAY'?expiration!==today:expiration<=today))return;
 return h;
}
export function displayStructure(h:Horizon|undefined,fresh:boolean) {
 if(!fresh||!h)return undefined;
 const s=h.structure;
 return s.provisional??(s.complete?s:undefined);
}
export function offSession(current:OffSummary|undefined,history:OffSummary[]=[],selected=''):OffSummary|undefined {
 const sessions=[...(current?[current]:[]),...history].sort((a,b)=>b.date.localeCompare(a.date));
 return sessions.find(s=>s.date===selected)??sessions.find(s=>s.print_count>0)??current;
}
