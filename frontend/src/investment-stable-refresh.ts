// Compare generated source, not browser-serialized HTML or user-mutated attributes.
const sources=new WeakMap<Element,string>();
const identities=new WeakMap<Element,string>();
export function refreshInvestmentRegion(el:HTMLElement,html:string,identity=''):boolean {
 if(sources.get(el)===html&&identities.get(el)===identity)return false;
 const same=identities.get(el)===identity;
 const key=(d:Element)=>d.querySelector('summary')?.textContent?.trim()??'';
 const expanded=new Map<string,boolean>();
 if(same)el.querySelectorAll<HTMLDetailsElement>('details').forEach((d,i)=>expanded.set(i+'|'+key(d),d.open));
 const selectors=['[data-ir-trade-date]','[data-ir-trade-session]','[data-ir-trade-min]'];
 const values=same?selectors.map(selector=>({selector,value:el.querySelector<HTMLInputElement|HTMLSelectElement>(selector)?.value})):[];
 const anchor=same?el.querySelector<HTMLElement>('.ir-detail'):null;
 const top=anchor?.getBoundingClientRect().top;
 const x=window.scrollX,y=window.scrollY;
 el.innerHTML=html;
 sources.set(el,html);identities.set(el,identity);
 if(same){
  el.querySelectorAll<HTMLDetailsElement>('details').forEach((d,i)=>{const open=expanded.get(i+'|'+key(d));if(open!=null)d.open=open;});
  values.forEach(({selector,value})=>{const input=el.querySelector<HTMLInputElement|HTMLSelectElement>(selector);if(input&&value!=null)input.value=value;});
  const next=el.querySelector<HTMLElement>('.ir-detail');
  // Compensate only for a same-company update. Opening a new company scrolls intentionally.
  const shift=top!=null&&next?next.getBoundingClientRect().top-top:0;
  if(window.scrollY!==y||shift!==0)window.scrollTo({left:x,top:y+shift,behavior:'instant'});
 }
 return true;
}
