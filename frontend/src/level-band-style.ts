/** Display convention only: existing structural importance, never a new signal or probability. */
export interface ColoredMapLevel {price:number;name:string;importance?:number|null}
export const LEVEL_BAND_HEIGHT=24;
export function levelBandStyle(level:ColoredMapLevel){
 const score=typeof level.importance==='number'&&Number.isFinite(level.importance)&&level.importance>=0&&level.importance<=100?level.importance:null;
 const rgb=/Main Magnet/.test(level.name)?'61,215,187':/Call Wall/.test(level.name)?'174,139,233':/Put Wall/.test(level.name)?'83,161,232':'89,193,174';
 const alpha=score==null?.045:.08+.28*score/100;
 const label=level.name.replace('Main Magnet','IMÁN PRINCIPAL').replace('Call Wall','CALL WALL').replace('Put Wall','PUT WALL');
 return {height:LEVEL_BAND_HEIGHT,rgb,fill:`rgba(${rgb},${alpha})`,edge:`rgba(${rgb},${score==null?.16:.25+.35*score/100})`,label:`${label} · ${score==null?'importancia pendiente':score.toFixed(0)+'/100'}`};
}
