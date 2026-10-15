/** Reconcile incoming observations without detaching the workspace or its controls. */
export function updateLiveWorkspace(root:Element,html:string):void {
 const template=document.createElement('template');
 template.innerHTML=html;
 const next=template.content.firstElementChild;
 if(next)patch(root,next);
}
/** Update a panel's content while retaining its controls and existing nodes. */
export function updatePanel(root:Element,html:string):void {
 const template=document.createElement('template');
 template.innerHTML=html;
 const next=root.cloneNode(false) as Element;
 next.appendChild(template.content);
 patch(root,next);
}
function key(node:Node):string|null {
 if(!(node instanceof Element))return null;
 for(const name of ['id','data-horizon-details','data-slot-input','data-asset-slot','data-watchlist-symbol','data-scanner-period','data-bot-trade']){
  if(node.hasAttribute(name))return `${node.nodeName}:${name}:${node.getAttribute(name)}`;
 }
 return null;
}
function compatible(old:Node,next:Node):boolean {
 return old.nodeType===next.nodeType&&old.nodeName===next.nodeName&&key(old)===key(next);
}
function patch(old:Node,next:Node):void {
 if(!compatible(old,next)){old.parentNode?.replaceChild(next,old);return;}
 if(!(old instanceof Element)||!(next instanceof Element)){
  if(old.nodeValue!==next.nodeValue)old.nodeValue=next.nodeValue;
  return;
 }
 // Chart renderers own attributes as well as children (canvas sizing and SVG
 // viewBox). Reconciliation must not clear either between market frames.
 if(old.hasAttribute('data-chart-preserve'))return;
 const focused=document.activeElement===old;
 const preserve=(name:string)=>old instanceof HTMLDetailsElement&&name==='open'||focused&&['value','checked','selected'].includes(name);
 for(const attr of [...old.attributes])if(!preserve(attr.name)&&!next.hasAttribute(attr.name))old.removeAttribute(attr.name);
 for(const attr of [...next.attributes])if(!preserve(attr.name)&&old.getAttribute(attr.name)!==attr.value)old.setAttribute(attr.name,attr.value);
 if(old instanceof HTMLInputElement&&next instanceof HTMLInputElement&&!focused){
  if(old.value!==next.value)old.value=next.value;
  if(old.checked!==next.checked)old.checked=next.checked;
 }
 if(old instanceof HTMLTextAreaElement&&next instanceof HTMLTextAreaElement){
  if(!focused&&old.value!==next.value)old.value=next.value;
  return;
 }
 const selectedValue=next instanceof HTMLSelectElement&&old instanceof HTMLSelectElement?(focused?old.value:next.value):null;
 // Each sibling is updated in place. Keyed rows retain identity when reordered.
 let cursor=old.firstChild;
 for(const incoming of [...next.childNodes]){
  if(cursor&&compatible(cursor,incoming)){
   const following=cursor.nextSibling;patch(cursor,incoming);cursor=following;
  }else{
   const wanted=key(incoming);
   const match=wanted?[...old.childNodes].find(n=>key(n)===wanted):undefined;
   if(match){old.insertBefore(match,cursor);patch(match,incoming);}
   else old.insertBefore(incoming,cursor);
  }
 }
 while(cursor){const following=cursor.nextSibling;old.removeChild(cursor);cursor=following;}
 if(old instanceof HTMLSelectElement&&selectedValue!=null&&old.value!==selectedValue)old.value=selectedValue;
}
