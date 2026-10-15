let observer:ResizeObserver|undefined;
let frame=0;
let overviewHeight:number|undefined;
export function levelCardPositions(heights:number[]):{column:number;top:number}[]{
 const bottoms=[0,40],counts=[0,0],limit=Math.ceil(heights.length/2);
 return heights.map(height=>{
  const column=counts[0]>=limit?1:counts[1]>=limit?0:bottoms[0]<=bottoms[1]?0:1;
  const top=bottoms[column];counts[column]++;bottoms[column]+=Math.max(0,height)+12;
  return {column,top};
 });
}
/** Fixed outer map; balanced placement scrolls inside without resizing it. */
export function bindLevelCardSpacing():void {
 observer?.disconnect();cancelAnimationFrame(frame);
 const track=document.querySelector<HTMLElement>('[data-live-view="detailed"] .levels-route-design .day-map-track');
 if(!track)return;
 const layout=track.closest<HTMLElement>('.quick-command-layout.route-command-layout');
 const stack=layout?.querySelector<HTMLElement>(':scope > .auction-map-stack');
 if(layout&&stack){
  // Freeze the existing right-hand stack height, not a height derived from card counts.
  overviewHeight??=Math.ceil(stack.getBoundingClientRect().height);
  if(overviewHeight>0){layout.style.setProperty('--level-overview-height',`${overviewHeight}px`);layout.classList.add('level-overview-fixed');}
 }
 const cards=Array.from(track.querySelectorAll<HTMLElement>(':scope > .day-map-node'));
 const place=()=>{
  levelCardPositions(cards.map(card=>card.offsetHeight)).forEach((position,i)=>{
   cards[i].style.top=`${position.top}px`;
   cards[i].dataset.levelColumn=String(position.column);
  });
 };
 track.style.removeProperty('height');track.classList.add('level-cards-close');
 track.tabIndex=0;track.setAttribute('aria-label','Niveles importantes; desplazamiento dentro del mapa');
 place();observer=new ResizeObserver(()=>{cancelAnimationFrame(frame);frame=requestAnimationFrame(place);});
 observer.observe(track);cards.forEach(card=>observer!.observe(card));
}
