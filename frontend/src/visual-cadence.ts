/** Coalesce visual updates only. Callers retain every incoming market snapshot. */
export class VisualCadence {
 private last=-Infinity;
 private timer:ReturnType<typeof setTimeout>|undefined;
 private latest:(()=>void)|undefined;
 constructor(private interval=1000){}
 run(render:()=>void,immediate=false){
  this.latest=render;
  const remaining=this.interval-(Date.now()-this.last);
  if(immediate||remaining<=0){this.flush();return;}
  if(this.timer==null)this.timer=setTimeout(()=>this.flush(),remaining);
 }
 cancel(){if(this.timer!=null)clearTimeout(this.timer);this.timer=undefined;this.latest=undefined;}
 private flush(){const render=this.latest;this.cancel();this.last=Date.now();render?.();}
}
