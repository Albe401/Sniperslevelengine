let observer:ResizeObserver|undefined;
let frame=0;
export function levelCardPositions(heights:number[],availableHeight=0):{column:number;top:number;height:number}[]{
 const bottoms=[0,0],counts=[0,0],limit=Math.ceil(heights.length/2);
 const positions=heights.map(height=>{
  const column=counts[0]>=limit?1:counts[1]>=limit?0:bottoms[0]<=bottoms[1]?0:1;
  const top=bottoms[column];counts[column]++;bottoms[column]+=Math.max(0,height)+12;
  return {column,top,height:Math.max(0,height)};
 });
 const extra=counts.map((count,column)=>count?Math.max(0,availableHeight-(bottoms[column]-12))/count:0);
 const cursor=[0,0];
 return positions.map(p=>{const top=cursor[p.column],height=p.height+extra[p.column];cursor[p.column]+=height+12;return {column:p.column,top,height};});
}
/** Shared panel height follows the targets column; cards fill its remaining space. */
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
  // Measure the intrinsic column, not its stretched 100% height (which includes the hole).
  const height=flow.style.height,maxHeight=flow.style.maxHeight;
  flow.style.height='auto';flow.style.maxHeight='none';
  const css=getComputedStyle(panel);
  const inset=[css.paddingTop,css.paddingBottom,css.borderTopWidth,css.borderBottomWidth].reduce((n,v)=>n+(parseFloat(v)||0),0);
  const fitted=Math.ceil(flow.getBoundingClientRect().height+inset);
  flow.style.height=height;flow.style.maxHeight=maxHeight;
  if(fitted>inset){layout.style.setProperty('--level-overview-height',`${fitted}px`);layout.classList.add('level-overview-fixed');}
 };
 const cards=Array.from(track.querySelectorAll<HTMLElement>(':scope > .day-map-node'));
 const place=()=>{
  fitOverview();
  // Measure content first; previously assigned space must not accumulate on refresh.
  cards.forEach(card=>card.style.removeProperty('min-height'));
  levelCardPositions(cards.map(card=>card.offsetHeight),track.clientHeight).forEach((position,i)=>{
   cards[i].style.top=`${position.top}px`;
   cards[i].style.minHeight=`${position.height}px`;
   cards[i].dataset.levelColumn=String(position.column);
  });
 };
 track.style.removeProperty('height');track.classList.add('level-cards-close');
 track.tabIndex=0;track.setAttribute('aria-label','Niveles importantes; desplazamiento dentro del mapa');
 place();observer=new ResizeObserver(()=>{cancelAnimationFrame(frame);frame=requestAnimationFrame(place);});
 observer.observe(track);cards.forEach(card=>observer!.observe(card));
 if(flow){observer.observe(flow);Array.from(flow.children).forEach(child=>observer!.observe(child));}
}
