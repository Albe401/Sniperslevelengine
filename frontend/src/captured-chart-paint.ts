/** Archived chart data stays fixed between captures. Geometry changes still paint
 * at the existing cadence; the wall-clock tick retains expiry/status updates. */
export class CapturedChartPaint {
 private last='';
 invalidate(){this.last='';}
 shouldPaint(captured:boolean,visible:boolean,geometry:readonly unknown[],now:number){
  if(!captured)return true;
  if(!visible){this.last='';return false;}
  const key=JSON.stringify([geometry,Math.floor(now/1000)]);
  if(key===this.last)return false;
  this.last=key;return true;
 }
}
