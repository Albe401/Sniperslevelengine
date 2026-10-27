import {captureReadingScroll} from './reading-scroll';
let observer:ResizeObserver|undefined;
let frame=0;
/** Preserve the measured map height after the CVD card was removed from the web layout. */
export function retainedCvdSpace(flowWidth:number):number {
 // The former card added 166px at normal widths, 180px when its text wrapped
 // in a narrow flow column. These include its margins and the column gap.
 return Number.isFinite(flowWidth)&&flowWidth<=264?180:166;
}
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
/** Shared panel height follows the targets column; cards move within it. */
export function bindLevelCardSpacing():void {
 observer?.disconnect();cancelAnimationFrame(frame);
 const track=document.querySelector<HTMLElement>('[data-live-view="detailed"] .levels-route-design .day-map-track');
 if(!track)return;
 const layout=track.closest<HTMLElement>('.quick-command-layout.route-command-layout');
 const stack=layout?.querySelector<HTMLElement>(':scope > .auction-map-stack');
 const panel=track.closest<HTMLElement>('.levels-route-design');
 const flow=panel?.querySelector<HTMLElement>(':scope > .flow-column');
 const fitOverview=()=>{
  if(!layout||!stack||!panel||!flow)return;
  const height=flow.style.height,maxHeight=flow.style.maxHeight;
  flow.style.height='auto';flow.style.maxHeight='none';
  const css=getComputedStyle(panel);
  const inset=[css.paddingTop,css.paddingBottom,css.borderTopWidth,css.borderBottomWidth].reduce((n,v)=>n+(parseFloat(v)||0),0);
  const natural=flow.getBoundingClientRect();
  const reserve=flow.querySelector('.cvd-panel')?0:retainedCvdSpace(natural.width);
  const fitted=Math.max(824,Math.ceil(natural.height+inset+reserve));
  flow.style.height=height;flow.style.maxHeight=maxHeight;
  if(fitted>inset){layout.style.setProperty('--level-overview-height',`${fitted}px`);layout.classList.add('level-overview-fixed');}
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
