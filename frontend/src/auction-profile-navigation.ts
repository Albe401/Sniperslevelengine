/** View state only. Never changes auction bins, levels, scores or market subscriptions. */
export interface ProfileRange {lo:number;hi:number}
const views=new Map<string,ProfileRange>();
export function auctionProfileRange(key:string,fit:ProfileRange,minimumSpan=0):ProfileRange {
 const saved=views.get(key);if(!saved)return fit;
 const span=Math.max(saved.hi-saved.lo,minimumSpan),center=(saved.lo+saved.hi)/2;
 return {lo:center-span/2,hi:center+span/2};
}
export function moveAuctionProfile(key:string,fit:ProfileRange,fraction:number,start=auctionProfileRange(key,fit)):ProfileRange {
 if(!Number.isFinite(fraction)||!Number.isFinite(start.lo)||!Number.isFinite(start.hi)||start.hi<=start.lo)return start;
 const span=start.hi-start.lo,center=Math.max(fit.lo,Math.min(fit.hi,(start.lo+start.hi)/2+fraction*span));
 const next={lo:center-span/2,hi:center+span/2};views.set(key,next);return next;
}
export function resetAuctionProfile(key?:string){if(key)views.delete(key);else views.clear();}

export function bindAuctionProfileNavigation(refresh:()=>void,busy:(value:boolean)=>void){
 document.querySelectorAll<HTMLElement>('[data-auction-profile-key]').forEach(plot=>{
  const key=plot.dataset.auctionProfileKey!,fit={lo:Number(plot.dataset.fitLo),hi:Number(plot.dataset.fitHi)};
  const range=()=>({lo:Number(plot.dataset.viewLo),hi:Number(plot.dataset.viewHi)});
  const paint=(next:ProfileRange)=>{
   const old=range(),span=old.hi-old.lo,height=plot.clientHeight;
   if(span<=0||height<=0)return;
   const pixels=(next.hi-old.hi)/span*height;
   plot.querySelectorAll<HTMLElement>('.auction-profile-bars,.auction-profile-axis').forEach(layer=>{layer.style.transform=`translateY(${pixels}px)`;});
  };
  let drag:{id:number;y:number;start:ProfileRange;next:ProfileRange}|undefined;
  const finish=()=>{if(!drag)return;const id=drag.id;drag=undefined;plot.classList.remove('is-panning');if(plot.hasPointerCapture(id))plot.releasePointerCapture(id);busy(false);refresh();};
  plot.onpointerdown=e=>{
   if(e.button!==0||e.isPrimary===false)return;e.preventDefault();e.stopPropagation();
   drag={id:e.pointerId,y:e.clientY,start:range(),next:range()};plot.setPointerCapture(e.pointerId);plot.classList.add('is-panning');busy(true);
  };
  plot.onpointermove=e=>{
   if(!drag||e.pointerId!==drag.id)return;e.preventDefault();
   drag.next=moveAuctionProfile(key,fit,(e.clientY-drag.y)/Math.max(1,plot.clientHeight),drag.start);paint(drag.next);
  };
  plot.onpointerup=finish;plot.onpointercancel=finish;plot.onlostpointercapture=finish;
  plot.onwheel=e=>{
   if(drag||!Number.isFinite(e.deltaY)||e.deltaY===0)return;e.preventDefault();e.stopPropagation();
   const delta=e.deltaY*(e.deltaMode===1?22:e.deltaMode===2?plot.clientHeight:1);
   moveAuctionProfile(key,fit,-delta/Math.max(1,plot.clientHeight),range());refresh();
  };
  plot.onkeydown=e=>{
   if(!['ArrowUp','ArrowDown','Home'].includes(e.key))return;e.preventDefault();e.stopPropagation();
   if(e.key==='Home')resetAuctionProfile(key);else moveAuctionProfile(key,fit,e.key==='ArrowUp'?.15:-.15,range());refresh();
  };
  plot.closest('.auction-volume-profile')?.querySelectorAll<HTMLButtonElement>('[data-auction-pan]').forEach(button=>{
   button.onclick=()=>{if(button.dataset.auctionPan==='reset')resetAuctionProfile(key);else moveAuctionProfile(key,fit,Number(button.dataset.auctionPan),range());refresh();};
  });
 });
}
