/** Retain the reader's viewport across synchronous data/layout updates.
 * Scoped to the same asset; GEX also requires the same expiry/session/page.
 * No timers, provider calls, or market values are stored here.
 */
export function captureReadingScroll(root:ParentNode=document):()=>void {
 const layout=root.querySelector<HTMLElement>('.quick-command-layout[data-reading-symbol]');
 if(!layout)return ()=>{};
 const symbol=layout.dataset.readingSymbol,view=layout.closest('[data-live-view]')?.getAttribute('data-live-view');
 const page={x:window.scrollX,y:window.scrollY};
 const modal=layout.querySelector<HTMLDetailsElement>('.structural-enrichment[open]');
 const detail=modal?{top:modal.scrollTop,left:modal.scrollLeft,returnTarget:modal.dataset.structuralReturn,expanded:new Set([...modal.querySelectorAll<HTMLDetailsElement>('[data-horizon-details][open]')].map(d=>d.dataset.horizonDetails))}:null;
 const selectors=['.day-map-track','.flow-column','.auction-map-stack','.massive-exposure-table'];
 const positions=selectors.flatMap(selector=>{
  const element=layout.querySelector<HTMLElement>(selector);if(!element)return [];
  const panel=element.closest<HTMLElement>('[data-exposure-key]');
  return [{selector,top:element.scrollTop,left:element.scrollLeft,key:panel?.dataset.exposureKey,index:panel?.dataset.exposurePageIndex}];
 });
 return ()=>{
  const next=root.querySelector<HTMLElement>('.quick-command-layout[data-reading-symbol]');
  if(!next||next.dataset.readingSymbol!==symbol||next.closest('[data-live-view]')?.getAttribute('data-live-view')!==view)return;
  for(const position of positions){
   const element=next.querySelector<HTMLElement>(position.selector);if(!element)continue;
   const panel=element.closest<HTMLElement>('[data-exposure-key]');
   if(panel?.dataset.exposureKey!==position.key||panel?.dataset.exposurePageIndex!==position.index)continue;
   element.scrollTop=position.top;element.scrollLeft=position.left;
  }
  if(detail){const panel=next.querySelector<HTMLDetailsElement>('.structural-enrichment');if(panel){panel.open=true;if(detail.returnTarget!==undefined)panel.dataset.structuralReturn=detail.returnTarget;
   panel.querySelectorAll<HTMLDetailsElement>('[data-horizon-details]').forEach(d=>{d.open=detail.expanded.has(d.dataset.horizonDetails);});panel.scrollTop=detail.top;panel.scrollLeft=detail.left;
  }}
  window.scrollTo({top:page.y,left:page.x,behavior:'instant'});
 };
}
