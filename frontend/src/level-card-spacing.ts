import {captureReadingScroll} from './reading-scroll';
let observer:ResizeObserver|undefined;
let frame=0;
let resizeListener:(()=>void)|undefined;
let retainedColumns:{key:string;columns:number[]}|undefined;
/** Shared height of the complete levels + GEX + structure overview. */
export const detailedOverviewHeight=824;
export function levelOverviewHeight(...contentHeights:number[]):number {
 return Math.ceil(Math.max(detailedOverviewHeight,...contentHeights.filter(Number.isFinite)));
}
export function levelCardPositions(heights:number[],availableHeight=0,fixedColumns?:number[]):{column:number;top:number;height:number}[]{
 const bottoms=[0,0],counts=[0,0],gap=6;
 const sizes=heights.map(v=>Number.isFinite(v)?Math.max(0,v):0);
 // Small maps can use the best height partition, avoiding greedy overflow
 // when a tall CALL/PUT card arrives after several short cards.
 let assignment:number[]|undefined=fixedColumns?.length===sizes.length&&fixedColumns.every(v=>v===0||v===1)?fixedColumns:undefined;
 if(!assignment&&sizes.length>1&&sizes.length<=12){
  let best=Infinity;
  for(let mask=1;mask<2**(sizes.length-1);mask++){
   const totals=[0,0],candidate=sizes.map((_,i)=>i===0?0:(mask>>(i-1))&1);
   sizes.forEach((height,i)=>{totals[candidate[i]]+=height+gap;});
   const score=Math.max(...totals);
   if(score<best){best=score;assignment=candidate;}
  }
 }
 // Assign by occupied height, never by a fixed count of cards per side.
 const positions=heights.map((value,index)=>{
  const height=Number.isFinite(value)?Math.max(0,value):0;
  const column=assignment?.[index]??(bottoms[0]<=bottoms[1]?0:1);
  const top=bottoms[column];counts[column]++;bottoms[column]+=height+gap;
  return {column,top,height};
 });
 // Use the restored rail down to its lower edge; never shrink natural content.
 const extra=counts.map((count,column)=>count?Math.max(0,availableHeight-(bottoms[column]-gap))/count:0);
 const cursor=[0,0];
 return positions.map(p=>{const top=cursor[p.column],height=p.height+extra[p.column];cursor[p.column]+=height+gap;return {...p,top,height};});
}
/** A fixed rail with all cards visible; only the page and GEX table may scroll. */
export function bindLevelCardSpacing():void {
 observer?.disconnect();cancelAnimationFrame(frame);
 if(resizeListener)window.removeEventListener('resize',resizeListener);
 const track=document.querySelector<HTMLElement>('[data-live-view="detailed"] .levels-route-design .day-map-track');
 if(!track)return;
 const layout=track.closest<HTMLElement>('.quick-command-layout.route-command-layout');
 const stack=layout?.querySelector<HTMLElement>(':scope > .auction-map-stack');
 const panel=track.closest<HTMLElement>('.levels-route-design');
 const flow=panel?.querySelector<HTMLElement>(':scope > .flow-column');
 const map=track.closest<HTMLElement>('.day-map');
 const numeric=(value:string)=>parseFloat(value)||0;
 const verticalInsets=(element:HTMLElement)=>{const s=getComputedStyle(element);return [s.paddingTop,s.paddingBottom,s.borderTopWidth,s.borderBottomWidth].reduce((sum,v)=>sum+numeric(v),0);};
 const naturalHeight=(element:HTMLElement)=>{
  const saved=element.style.cssText;
  element.style.height='auto';element.style.minHeight='0';element.style.maxHeight='none';element.style.alignSelf='start';
  const height=element.offsetHeight;
  element.style.cssText=saved;
  return height;
 };
 const fitOverview=(extent:number)=>{
  if(!layout||!stack||!panel)return;
  // Measure intrinsic content, not scrollHeight of a box already stretched to H.
  const mapStyle=map?getComputedStyle(map):null;
  const children=map?Array.from(map.children).filter((c):c is HTMLElement=>c instanceof HTMLElement):[];
  const chrome=children.filter(c=>c!==track).reduce((sum,c)=>{const s=getComputedStyle(c);return sum+c.offsetHeight+numeric(s.marginTop)+numeric(s.marginBottom);},0)+(map?verticalInsets(map):0)+Math.max(0,children.length-1)*numeric(mapStyle?.rowGap??'0');
  const neededMap=extent+verticalInsets(track)+chrome+verticalInsets(panel);
  const neededFlow=flow?naturalHeight(flow)+verticalInsets(panel):0;
  const neededStack=naturalHeight(stack);
  const snapshot=layout.closest('#snapshot-image-root');
  const current=snapshot?parseFloat(layout.style.getPropertyValue('--level-overview-height'))||0:0;
  layout.style.setProperty('--level-overview-height',`${levelOverviewHeight(neededMap,neededFlow,neededStack,current)}px`);
  layout.classList.add('level-overview-fixed');
 };
 const cards=Array.from(track.querySelectorAll<HTMLElement>(':scope > .day-map-node'));
 const columnKey=[layout?.dataset.readingSymbol??'',...cards.map(c=>c.querySelector(':scope > .day-map-level > span:not(.day-map-symbol)')?.textContent??c.className)].join('|');
 const positionDetail=(detail:HTMLDetailsElement)=>{
  if(!detail.open)return;
  const summary=detail.querySelector('summary');if(!summary)return;
  const rect=summary.getBoundingClientRect(),above=Math.max(80,rect.top-12),below=Math.max(80,window.innerHeight-rect.bottom-12);
  detail.dataset.readingSide=above>below?'up':'down';
  detail.style.setProperty('--reading-detail-height',`${Math.min(360,Math.max(above,below))}px`);
 };
 const details=Array.from(track.querySelectorAll<HTMLDetailsElement>('.map-level-reading'));
 details.forEach(detail=>{detail.querySelector<HTMLElement>('.map-reading-detail')?.setAttribute('tabindex','0');detail.addEventListener('toggle',()=>positionDetail(detail));});
 const place=()=>{
  const restoreScroll=captureReadingScroll();
  cards.forEach(card=>card.style.removeProperty('min-height'));
  const heights=cards.map(card=>card.offsetHeight);
  const columns=retainedColumns?.key===columnKey?retainedColumns.columns:undefined;
  const natural=levelCardPositions(heights,0,columns);
  retainedColumns={key:columnKey,columns:natural.map(p=>p.column)};
  fitOverview(Math.max(0,...natural.map(p=>p.top+p.height)));
  levelCardPositions(heights,track.clientHeight,retainedColumns.columns).forEach((position,i)=>{
   cards[i].style.top=`${position.top}px`;
   cards[i].style.minHeight=`${position.height}px`;
   cards[i].dataset.levelColumn=String(position.column);
  });
  restoreScroll();
  details.forEach(positionDetail);
 };
 track.style.removeProperty('height');track.classList.add('level-cards-close');
 track.setAttribute('aria-label','Niveles importantes fijos; todos los cuadros visibles, sin desplazamiento interno');
 place();observer=new ResizeObserver(()=>{cancelAnimationFrame(frame);frame=requestAnimationFrame(place);});
 observer.observe(track);cards.forEach(card=>observer!.observe(card));
 if(flow){observer.observe(flow);Array.from(flow.children).forEach(child=>observer!.observe(child));}
 if(map)Array.from(map.children).filter(c=>c!==track).forEach(child=>observer!.observe(child));
 if(stack){observer.observe(stack);Array.from(stack.children).forEach(child=>observer!.observe(child));}
 resizeListener=()=>{cancelAnimationFrame(frame);frame=requestAnimationFrame(place);};window.addEventListener('resize',resizeListener);
}
