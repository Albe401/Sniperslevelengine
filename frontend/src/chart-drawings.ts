import {escapeHtml as esc} from './model';
import type {ChartGeometry} from './chart';
export type DrawTool='select'|'pan'|'horizontal'|'trend'|'ray'|'block'|'rectangle'|'measure';
export interface Anchor {time:number;price:number}
export interface Drawing {id:string;kind:Exclude<DrawTool,'select'|'pan'>;a:Anchor;b:Anchor;color:string}
export interface DrawingState {rows:Drawing[];undo:Drawing[][];redo:Drawing[][];selected:string|null;tool:DrawTool;color:string;hidden:boolean;locked:boolean;error:string}
const cache=new Map<string,DrawingState>();
let drawingSequence=0;
export function drawingId():string {
 return globalThis.crypto?.randomUUID?.()??`mark-${Date.now()}-${++drawingSequence}-${Math.random().toString(36).slice(2)}`;
}
const colors=['#24dfff','#16f6a2','#ff496e','#ffdc52','#ce7bff'];
export function validateDrawings(value:unknown):Drawing[]{
 if(!Array.isArray(value)||value.length>200)return [];
 const ids=new Set<string>();
 return value.filter((r):r is Drawing=>{
  if(!r||typeof r!=='object')return false;
  const d=r as Drawing;
  const point=(p:Anchor)=>p&&Number.isFinite(p.time)&&p.time>0&&Number.isFinite(p.price)&&p.price>0;
  if(typeof d.id!=='string'||d.id.length>80||ids.has(d.id)||!['horizontal','trend','ray','block','rectangle','measure'].includes(d.kind)||!colors.includes(d.color)||!point(d.a)||!point(d.b))return false;
  ids.add(d.id);return true;
 }).map(d=>({...d,a:{...d.a},b:{...d.b}}));
}
export function drawingState(symbol:string):DrawingState {
 let s=cache.get(symbol);if(s)return s;
 let rows:Drawing[]=[];let error='';
 try{rows=validateDrawings(JSON.parse(localStorage.getItem(`snipers.drawings.v1.${symbol}`)??'[]'));}catch{error='No se pudieron recuperar las marcas locales.';}
 s={rows,undo:[],redo:[],selected:null,tool:'pan',color:colors[0],hidden:false,locked:false,error};cache.set(symbol,s);return s;
}
export function commitDrawing(symbol:string,rows:Drawing[]):void {
 const s=drawingState(symbol);s.undo.push(structuredClone(s.rows));s.undo=s.undo.slice(-30);s.redo=[];s.rows=validateDrawings(rows);save(symbol);
}
function save(symbol:string){const s=drawingState(symbol);try{localStorage.setItem(`snipers.drawings.v1.${symbol}`,JSON.stringify(s.rows));s.error='';}catch{s.error='No se pudieron guardar las marcas. Permanecen en memoria.';}}
export function undoDrawing(symbol:string,redo=false){const s=drawingState(symbol),from=redo?s.redo:s.undo,to=redo?s.undo:s.redo;const rows=from.pop();if(!rows)return;to.push(structuredClone(s.rows));s.rows=rows;s.selected=null;save(symbol);}
/** Map actual candle timestamps, interpolating between them; drawing anchors survive zoom and gaps. */
export function timeToIndex(times:number[],time:number):number {
 if(times.length<2)return 0;
 let i=times.findIndex(t=>t>=time);if(i===-1)i=times.length-1;if(i===0)i=1;
 return i-1+(time-times[i-1])/(times[i]-times[i-1]);
}
export function indexToTime(times:number[],index:number):number {
 if(times.length<2)return times[0]??0;
 const i=Math.max(0,Math.min(times.length-2,Math.floor(index)));
 return times[i]+(index-i)*(times[i+1]-times[i]);
}
export function drawingToolbar(symbol:string,height:number):string {
 const s=drawingState(symbol);
 return `<div class="drawing-toolbar" aria-label="Herramientas de dibujo">${([['pan','Mover'],['select','Seleccionar'],['horizontal','HLine · Horizontal'],['trend','Tendencia'],['ray','Rayo'],['block','Order block'],['rectangle','Rectángulo']] as const).map(([tool,label])=>`<button data-draw-tool="${tool}" aria-pressed="${s.tool===tool}">${label}</button>`).join('')}<label>Color <select data-draw-color aria-label="Color del dibujo">${colors.map((c,i)=>`<option value="${c}" ${c===s.color?'selected':''}>${['Cian','Verde','Rojo','Amarillo','Violeta'][i]}</option>`).join('')}</select></label><button data-draw-action="undo" ${s.undo.length?'':'disabled'}>Deshacer</button><button data-draw-action="redo" ${s.redo.length?'':'disabled'}>Rehacer</button><button data-draw-action="delete" ${s.selected&&!s.locked?'':'disabled'}>Eliminar marca</button><button data-draw-action="hide" aria-pressed="${s.hidden}">${s.hidden?'Mostrar marcas':'Ocultar marcas'}</button><button data-draw-action="lock" aria-pressed="${s.locked}">${s.locked?'Desbloquear':'Bloquear marcas'}</button><button data-draw-action="auto">Autoescala</button><label>Altura <input data-draw-height aria-label="Altura de la gráfica" type="range" min="320" max="1000" step="20" value="${height}"></label><span data-draw-status>${esc(s.error||(s.locked?'Marcas bloqueadas: pulsa Desbloquear para editarlas.':s.hidden?'Marcas ocultas: pulsa Mostrar marcas.':s.tool==='horizontal'?'HLine: un clic coloca la línea · Seleccionar para moverla':s.tool==='pan'?'Arrastra libremente en cualquier dirección · rueda: zoom · doble clic en precios: autoescala':'Arrastra para dibujar · Seleccionar para editar · Supr: borrar · Ctrl+Z: deshacer'))}</span></div>`;
}
export function drawingsSvg(rows:Drawing[],selected:string|null,g:ChartGeometry,times:number[]):string {
 const px=(p:Anchor)=>g.x(timeToIndex(times,p.time)),py=(p:Anchor)=>g.y(p.price);
 return rows.map(d=>{
  const ax=px(d.a),ay=py(d.a),bx=px(d.b),by=py(d.b);
  let shape='';
  if(d.kind==='horizontal')shape=`<line x1="${g.left}" x2="${g.candleEnd}" y1="${ay}" y2="${ay}"/>`;
  else if(d.kind==='trend'||d.kind==='ray'){
   const end=d.kind==='ray'?(bx>=ax?g.candleEnd:g.left):bx;
   const endY=d.kind==='ray'&&Math.abs(bx-ax)>.01?ay+(by-ay)*(end-ax)/(bx-ax):by;
   shape=`<line x1="${ax}" y1="${ay}" x2="${end}" y2="${endY}"/>`;
  }else shape=`<rect x="${Math.min(ax,bx)}" y="${Math.min(ay,by)}" width="${Math.max(2,Math.abs(bx-ax))}" height="${Math.max(2,Math.abs(by-ay))}" fill="${d.color}" fill-opacity=".14"/>${d.kind==='block'?`<text x="${Math.min(ax,bx)+5}" y="${Math.min(ay,by)+15}" fill="${d.color}" stroke="none" font-size="11">ORDER BLOCK · MANUAL</text>`:''}`;
  return `<g data-drawing-id="${esc(d.id)}" stroke="${d.color}" stroke-width="${d.id===selected?3:1.8}" fill="none" style="pointer-events:visiblePainted"><title>Marca manual · ${esc(d.kind)} · ${d.a.price.toFixed(2)} / ${d.b.price.toFixed(2)}</title>${['horizontal','trend','ray'].includes(d.kind)?`<g stroke="transparent" stroke-width="16" style="pointer-events:stroke">${shape}</g>`:''}${shape}${d.id===selected?(d.kind==='horizontal'?[['a',Math.max(g.left+16,Math.min(g.candleEnd-16,ax)),ay]]:[['a',ax,ay],['b',bx,by]]).map(([key,x,y])=>`<circle data-draw-handle="${key}" cx="${x}" cy="${y}" r="12" fill="transparent" stroke="none"/><circle data-draw-handle="${key}" cx="${x}" cy="${y}" r="6" fill="#07132b"/>`).join(''):''}</g>`;
 }).join('');
}
interface Hooks {preview?:()=>string;refresh:()=>void;busy:(v:boolean)=>void;pan:(bars:number,shift:number)=>void;zoom:(factor:number)=>void;scale:(range:[number,number]|undefined)=>void;height:(n:number)=>void}
export function bindDrawingTools(symbol:string,hooks:Hooks):void {
 const s=drawingState(symbol),root=document.querySelector<HTMLElement>('[data-live-view="gex"]');if(!root)return;
 const svg=root.querySelector<SVGSVGElement>('[data-gex-svg]');
 const refresh=()=>{hooks.busy(false);hooks.refresh();};
 root.querySelectorAll<HTMLButtonElement>('[data-draw-tool]').forEach(b=>b.onclick=()=>{s.tool=b.dataset.drawTool as DrawTool;refresh();});
 const color=root.querySelector<HTMLSelectElement>('[data-draw-color]');if(color)color.onchange=()=>{s.color=color.value;if(s.selected&&!s.locked)commitDrawing(symbol,s.rows.map(d=>d.id===s.selected?{...d,color:s.color}:d));refresh();};
 root.querySelectorAll<HTMLButtonElement>('[data-draw-action]').forEach(b=>b.onclick=()=>{const a=b.dataset.drawAction;if(a==='undo'||a==='redo')undoDrawing(symbol,a==='redo');if(a==='delete'&&!s.locked){commitDrawing(symbol,s.rows.filter(d=>d.id!==s.selected));s.selected=null;}if(a==='hide')s.hidden=!s.hidden;if(a==='lock')s.locked=!s.locked;if(a==='auto')hooks.scale(undefined);refresh();});
 const size=root.querySelector<HTMLInputElement>('[data-draw-height]');if(size){size.onpointerdown=()=>hooks.busy(true);size.oninput=()=>{const canvas=root.querySelector<HTMLElement>('.gex-canvas');if(canvas)canvas.style.height=size.value+'px';};size.onchange=()=>{hooks.height(Number(size.value));refresh();};size.onpointerup=()=>{hooks.height(Number(size.value));refresh();};size.onpointercancel=()=>refresh();}
 if(!svg)return;
 const times=(svg.dataset.times??'').split(',').map(Number),left=Number(svg.dataset.left),end=Number(svg.dataset.candleEnd),top=Number(svg.dataset.top),floor=Number(svg.dataset.floor),min=Number(svg.dataset.priceMin),max=Number(svg.dataset.priceMax),step=Number(svg.dataset.step)||(end-left)/times.length,viewStart=Number(svg.dataset.viewStart)||0;
 const g:ChartGeometry={left,end:Number(svg.dataset.end),candleEnd:end,top,floor,min,max,x:i=>left+(i-viewStart+.5)*step,y:p=>top+(max-p)/(max-min)*(floor-top)};
 const local=(e:MouseEvent)=>{const matrix=svg.getScreenCTM();return matrix?new DOMPoint(e.clientX,e.clientY).matrixTransform(matrix.inverse()):new DOMPoint();};
 const anchor=(p:DOMPoint):Anchor=>({time:indexToTime(times,(p.x-left)/step-.5+viewStart),price:Math.max(.0001,max-(p.y-top)/(floor-top)*(max-min))});
 let layer=svg.querySelector<SVGGElement>('[data-user-drawings]');
 if(layer)svg.append(layer);
 const paint=(rows=s.rows)=>{if(layer)layer.innerHTML=s.hidden?'':drawingsSvg(rows,s.selected,g,times);};paint();
 let start:DOMPoint|undefined,working:Drawing[]|undefined,id:string|undefined,handle:string|undefined,axis=false;
 let frame=0,pending:DOMPoint|undefined,lastPan:DOMPoint|undefined;
 const preview=()=>{
  if(!hooks.preview)return;
  const template=document.createElement('template');template.innerHTML=hooks.preview();
  const next=template.content.querySelector<SVGSVGElement>('[data-gex-svg]');if(!next)return;
  // Keep the capturing SVG and its handlers alive while redrawing the actual viewport.
  svg.innerHTML=next.innerHTML;
  for(const attr of Array.from(next.attributes))svg.setAttribute(attr.name,attr.value);
  layer=svg.querySelector('[data-user-drawings]');if(layer)svg.append(layer);
 };
 const moveViewport=(p:DOMPoint)=>{
  if(!start)return;
  if(axis){const center=(max+min)/2,span=(max-min)*Math.exp(Math.max(-2,Math.min(2,(p.y-start.y)/160)));hooks.scale([Math.max(.0001,center-span/2),center+span/2]);}
  else {const previous=lastPan??start;hooks.pan(-(p.x-previous.x)/step,(p.y-previous.y)/(floor-top)*(max-min));lastPan=p;}
  preview();
 };
 const flush=()=>{cancelAnimationFrame(frame);frame=0;if(pending){moveViewport(pending);pending=undefined;}};
 const cancel=()=>{cancelAnimationFrame(frame);frame=0;pending=undefined;start=undefined;working=undefined;refresh();};
 svg.style.cursor=s.tool==='pan'?'grab':s.tool==='select'?'default':'crosshair';
 svg.onpointerdown=e=>{
  if(e.button!==0)return;e.preventDefault();svg.focus({preventScroll:true});const p=local(e);if(p.y<top||p.y>floor)return;
  start=p;axis=p.x>g.end;hooks.busy(true);svg.setPointerCapture(e.pointerId);
  const target=e.target as Element;id=target.closest('[data-drawing-id]')?.getAttribute('data-drawing-id')??undefined;handle=target.getAttribute('data-draw-handle')??undefined;
  if(axis||s.tool==='pan'){lastPan=p;hooks.scale([min,max]);svg.style.cursor=axis?'ns-resize':'grabbing';return;}
  if(s.tool==='select'){s.selected=id??null;if(id&&!s.locked)working=structuredClone(s.rows);paint();return;}
  if(s.locked||s.hidden||p.x<left||p.x>end||s.rows.length>=200){cancel();return;}
  const a=anchor(p);id=drawingId();s.selected=id;working=[...structuredClone(s.rows),{id,kind:s.tool,a,b:{...a},color:s.color}];paint(working);
 };
 svg.onpointermove=e=>{
  const p=local(e);const crossX=svg.querySelector('[data-gex-cross-x]'),crossY=svg.querySelector('[data-gex-cross-y]');
  if(p.x>=left&&p.x<=g.end&&p.y>=top&&p.y<=floor){crossX?.setAttribute('visibility','visible');crossX?.setAttribute('x1',String(p.x));crossX?.setAttribute('x2',String(p.x));crossY?.setAttribute('visibility','visible');crossY?.setAttribute('y1',String(p.y));crossY?.setAttribute('y2',String(p.y));const label=root.querySelector('[data-gex-cursor]');if(label)label.textContent=`Precio ${anchor(p).price.toFixed(2)} · ${new Date(anchor(p).time).toLocaleString('es',{timeZone:'America/New_York'})} ET`;}
  if(start&&(axis||s.tool==='pan')){pending=p;if(!frame)frame=requestAnimationFrame(()=>{frame=0;flush();});return;}
  if(!start||!working||!id)return;
  const d=working.find(d=>d.id===id)!;
  if(s.tool==='select'){const original=s.rows.find(d=>d.id===id)!;if(handle==='a'||handle==='b'){d[handle]=anchor(p);if(d.kind==='horizontal')d.a.price=d.b.price=anchor(p).price;}else{const a0=anchor(start),a1=anchor(p),dt=a1.time-a0.time,dp=a1.price-a0.price;d.a={time:original.a.time+dt,price:Math.max(.0001,original.a.price+dp)};d.b={time:original.b.time+dt,price:Math.max(.0001,original.b.price+dp)};}}else {d.b=anchor(p);if(d.kind==='horizontal')d.a.price=d.b.price;}
  paint(working);
 };
 svg.onpointerup=e=>{
  if(!start)return;
  if(axis||s.tool==='pan'){pending=local(e);flush();}
  else if(working)commitDrawing(symbol,working);
  svg.style.cursor=s.tool==='pan'?'grab':'crosshair';
  start=undefined;working=undefined;refresh();
 };
 svg.onpointercancel=cancel;
 svg.onlostpointercapture=()=>{if(start)cancel();};
 svg.onwheel=e=>{e.preventDefault();if(start)return;const p=local(e);if(p.x>g.end||e.shiftKey){const center=(max+min)/2,span=(max-min)*(e.deltaY>0?1.15:.87);hooks.scale([Math.max(.0001,center-span/2),center+span/2]);}else hooks.zoom(e.deltaY>0?1.15:.87);refresh();};
 svg.onkeydown=e=>{if(e.key==='Escape'){e.preventDefault();s.tool='pan';cancel();}else if(e.key==='Delete'&&!s.locked&&s.selected){e.preventDefault();commitDrawing(symbol,s.rows.filter(d=>d.id!==s.selected));s.selected=null;refresh();}else if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='z'){e.preventDefault();undoDrawing(symbol,e.shiftKey);refresh();}};
 svg.ondblclick=e=>{if(local(e).x>g.end){hooks.scale(undefined);refresh();}};
}
