import {captureReadingScroll} from './reading-scroll';
let observer:ResizeObserver|undefined;
let frame=0;
/** Shared height of the complete levels + GEX + structure overview. */
export const detailedOverviewHeight=760;
export function levelCardPositions(heights:number[]):{column:number;top:number;height:number}[]{
 const bottoms=[0,0],gap=6;
 const sizes=heights.map(v=>Number.isFinite(v)?Math.max(0,v):0);
 // Small maps can use the best height partition, avoiding greedy overflow
 // when a tall CALL/PUT card arrives after several short cards.
 let assignment:number[]|undefined;
 if(sizes.length>1&&sizes.length<=12){
  let best=Infinity;
  for(let mask=1;mask<2**(sizes.length-1);mask++){
   const totals=[0,0],candidate=sizes.map((_,i)=>i===0?0:(mask>>(i-1))&1);
   sizes.forEach((height,i)=>{totals[candidate[i]]+=height+gap;});
   const score=Math.max(...totals);
   if(score<best){best=score;assignment=candidate;}
  }
 }
 // Assign by occupied height, never by a fixed count of cards per side.
 return heights.map((value,index)=>{
  const height=Number.isFinite(value)?Math.max(0,value):0;
  const column=assignment?.[index]??(bottoms[0]<=bottoms[1]?0:1);
  const top=bottoms[column];bottoms[column]+=height+gap;
  return {column,top,height};
 });
}
/** Keep the complete overview compact; overflowing readings remain scrollable. */
export function bindLevelCardSpacing():void {
 observer?.disconnect();cancelAnimationFrame(frame);
 const track=document.querySelector<HTMLElement>('[data-live-view="detailed"] .levels-route-design .day-map-track');
 if(!track)return;
 const layout=track.closest<HTMLElement>('.quick-command-layout.route-command-layout');
 const stack=layout?.querySelector<HTMLElement>(':scope > .auction-map-stack');
 const panel=track.closest<HTMLElement>('.levels-route-design');
 const flow=panel?.querySelector<HTMLElement>(':scope > .flow-column');
 const fitOverview=()=>{
  if(!layout||!stack||!panel)return;
  // Full snapshot images grow to include the right stack as well as the levels.
  const snapshot=layout.closest('#snapshot-image-root');
  const current=snapshot?parseFloat(layout.style.getPropertyValue('--level-overview-height'))||0:0;
  const fullStack=snapshot?stack.scrollHeight+stack.offsetHeight-stack.clientHeight:0;
  const fullFlow=snapshot&&flow?flow.scrollHeight+panel.offsetHeight-flow.clientHeight:0;
  layout.style.setProperty('--level-overview-height',`${Math.ceil(Math.max(detailedOverviewHeight,current,fullStack,fullFlow))}px`);
  layout.classList.add('level-overview-fixed');
  stack.tabIndex=0;
  stack.setAttribute('aria-label','Exposición GEX y mapa estructural; desplázate para ver toda la información');
 };
 const cards=Array.from(track.querySelectorAll<HTMLElement>(':scope > .day-map-node'));
 const place=()=>{
  const restoreScroll=captureReadingScroll();
  fitOverview();
  cards.forEach(card=>card.style.removeProperty('min-height'));
  // Keep natural content height; the panel's spare space must not stretch cards.
  levelCardPositions(cards.map(card=>card.offsetHeight)).forEach((position,i)=>{
   cards[i].style.top=`${position.top}px`;
   cards[i].dataset.levelColumn=String(position.column);
  });
  restoreScroll();
 };
 track.style.removeProperty('height');track.classList.add('level-cards-close');
 track.tabIndex=0;track.setAttribute('aria-label','Niveles importantes; distribución adaptable dentro del mapa');
 place();observer=new ResizeObserver(()=>{cancelAnimationFrame(frame);frame=requestAnimationFrame(place);});
 observer.observe(track);cards.forEach(card=>observer!.observe(card));
 if(flow){observer.observe(flow);Array.from(flow.children).forEach(child=>observer!.observe(child));}
}
