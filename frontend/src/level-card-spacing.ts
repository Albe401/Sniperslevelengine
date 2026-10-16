let observer:ResizeObserver|undefined;
let frame=0;
export function levelCardPositions(heights:number[],availableHeight=0):{column:number;top:number;height:number}[]{
 const bottoms=[0,0],counts=[0,0],gap=6;
 // Assign by occupied height, never by a fixed count of cards per side.
 const positions=heights.map(value=>{
  const height=Number.isFinite(value)?Math.max(0,value):0;
  const column=bottoms[0]<=bottoms[1]?0:1;
  const top=bottoms[column];counts[column]++;bottoms[column]+=height+gap;
  return {column,top,height};
 });
 // Move cards along the rail to use the available vertical space. Their content
 // stays compact; only positions change. Overflow remains readable if expanded.
 const spacing=counts.map((count,c)=>count>1?Math.max(gap,(availableHeight-(bottoms[c]-count*gap))/(count-1)):gap);
 const cursor=[0,0];
 return positions.map(p=>{const top=cursor[p.column];cursor[p.column]+=p.height+spacing[p.column];return {...p,top};});
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
  const fitted=Math.ceil(flow.getBoundingClientRect().height+inset);
  flow.style.height=height;flow.style.maxHeight=maxHeight;
  if(fitted>inset){layout.style.setProperty('--level-overview-height',`${fitted}px`);layout.classList.add('level-overview-fixed');}
 };
 const cards=Array.from(track.querySelectorAll<HTMLElement>(':scope > .day-map-node'));
 const place=()=>{
  fitOverview();
  cards.forEach(card=>card.style.removeProperty('min-height'));
  levelCardPositions(cards.map(card=>card.offsetHeight),track.clientHeight).forEach((position,i)=>{
   cards[i].style.top=`${position.top}px`;
   cards[i].dataset.levelColumn=String(position.column);
  });
 };
 track.style.removeProperty('height');track.classList.add('level-cards-close');
 track.tabIndex=0;track.setAttribute('aria-label','Niveles importantes; distribución adaptable dentro del mapa');
 place();observer=new ResizeObserver(()=>{cancelAnimationFrame(frame);frame=requestAnimationFrame(place);});
 observer.observe(track);cards.forEach(card=>observer!.observe(card));
 if(flow){observer.observe(flow);Array.from(flow.children).forEach(child=>observer!.observe(child));}
}
