import type {IChartApi,ISeriesApi,Logical} from 'lightweight-charts';
import {commitDrawing,drawingId,drawingState,indexToTime,timeToIndex,undoDrawing,type Anchor,type Drawing,type DrawTool} from './chart-drawings';
import {escapeHtml as esc} from './model';

export function measurement(d:Drawing,times:number[]):string {
 const change=d.b.price-d.a.price,percent=change/d.a.price*100;
 return `${change>=0?'+':''}$${change.toFixed(2)} · ${percent.toFixed(2)}% · ${Math.abs(timeToIndex(times,d.b.time)-timeToIndex(times,d.a.time)).toFixed(1)} velas`;
}
export function routeDrawingToolbar(symbol:string):string {
 const s=drawingState(symbol);
 const shapes:Record<string,string>={pan:'<path d="M12 3v18M3 12h18m-12-6 3-3 3 3m-6 12 3 3 3-3M6 9l-3 3 3 3m12-6 3 3-3 3"/>',select:'<path d="m5 3 13 9-6 1-3 6z"/>',horizontal:'<path d="M3 12h18"/><circle cx="4" cy="12" r="2"/><circle cx="20" cy="12" r="2"/>',trend:'<path d="m4 20 16-16"/><circle cx="4" cy="20" r="2"/><circle cx="20" cy="4" r="2"/>',rectangle:'<rect x="4" y="6" width="16" height="12" rx="1"/>',measure:'<path d="m4 17 13-13 4 4-13 13zM9 12l3 3m1-7 3 3"/>',undo:'<path d="m8 4-5 5 5 5M3 9h10a7 7 0 0 1 0 14"/>',delete:'<path d="M4 6h16M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7m4-7v7"/>'};
 const icon=(tool:string)=>`<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${shapes[tool]}</svg>`;
 return `<span class="route-draw-tools">${([['pan','Cursor / Mover'],['select','Seleccionar'],['horizontal','Horizontal'],['trend','Tendencia'],['rectangle','Zona'],['measure','Medir']] as const).map(([tool,label])=>`<button data-route-draw="${tool}" title="${label}" aria-label="${label}" aria-pressed="${s.tool===tool}">${icon(tool)}</button>`).join('')}<button data-route-edit="undo" title="Deshacer" aria-label="Deshacer" ${s.undo.length?'':'disabled'}>${icon('undo')}</button><button data-route-edit="delete" title="Borrar marca" aria-label="Borrar marca" ${s.selected?'':'disabled'}>${icon('delete')}</button></span><small class="route-draw-help" title="${esc(s.error||'Marcas manuales guardadas por activo · arrastra para dibujar · Esc: volver al cursor')}">${s.error?esc(s.error):'ⓘ'}</small>`;
}
export function drawingShapes(rows:Drawing[],selected:string|null,times:number[],x:(time:number)=>number|null,y:(price:number)=>number|null,width:number):string {
 return rows.map(d=>{
  const ax=x(d.a.time),bx=x(d.b.time),ay=y(d.a.price),by=y(d.b.price);
  if(ax==null||bx==null||ay==null||by==null||![ax,bx,ay,by].every(Number.isFinite))return '';
  const shape=d.kind==='horizontal'?`<line x1="0" x2="${width}" y1="${ay}" y2="${ay}"/>`:d.kind==='trend'||d.kind==='ray'?`<line x1="${ax}" x2="${bx}" y1="${ay}" y2="${by}"/>`:`<rect x="${Math.min(ax,bx)}" y="${Math.min(ay,by)}" width="${Math.max(2,Math.abs(bx-ax))}" height="${Math.max(2,Math.abs(by-ay))}" fill="${d.color}" fill-opacity=".12"/>`;
  return `<g data-route-mark="${esc(d.id)}" fill="none" stroke="${d.color}" stroke-width="${d.id===selected?3:1.5}"><title>Marca manual</title><g stroke="transparent" stroke-width="14">${shape}</g>${shape}${d.kind==='measure'?`<text x="${Math.min(ax,bx)+5}" y="${Math.min(ay,by)+15}" stroke="none" fill="${d.color}" font-size="11">${measurement(d,times)}</text>`:''}</g>`;
 }).join('');
}
/** Capturing overlay is kept alive during live updates; anchors use real timestamps and prices. */
export class RouteDrawingTools {
 private svg:SVGSVGElement;private symbol='';private times:number[]=[];private working:Drawing[]|undefined;private start:Anchor|undefined;private key='';
 constructor(private host:HTMLElement,private chart:IChartApi,private candles:ISeriesApi<'Candlestick'>){
  this.svg=document.createElementNS('http://www.w3.org/2000/svg','svg');this.svg.classList.add('route-user-drawings');host.append(this.svg);
  this.svg.onpointerdown=e=>{
   if(e.button!==0)return;const s=drawingState(this.symbol);if(s.tool==='pan')return;const a=this.anchor(e);if(!a)return;
   e.preventDefault();this.svg.setPointerCapture(e.pointerId);
   if(s.tool==='select'){s.selected=(e.target as Element).closest('[data-route-mark]')?.getAttribute('data-route-mark')??null;this.paint();return;}
   if(s.locked||s.hidden||s.rows.length>=200)return;
   this.start=a;const d:Drawing={id:drawingId(),kind:s.tool,a,b:{...a},color:s.color};s.selected=d.id;this.working=[...s.rows,d];this.paint();
  };
  this.svg.onpointermove=e=>{if(!this.working||!this.start)return;const a=this.anchor(e);if(!a)return;const d=this.working.at(-1)!;d.b=a;if(d.kind==='horizontal')d.a.price=a.price;this.paint();};
  this.svg.onpointerup=()=>{if(this.working)commitDrawing(this.symbol,this.working);this.working=undefined;this.start=undefined;this.paint();this.buttons();};
  this.svg.onpointercancel=()=>this.cancel();this.svg.onlostpointercapture=()=>{if(this.working)this.cancel();};
  host.addEventListener('keydown',this.keyboard);host.tabIndex=0;
 }
 private keyboard=(e:KeyboardEvent)=>{if(e.key==='Escape'){drawingState(this.symbol).tool='pan';this.cancel();this.buttons();}else if(e.key==='Delete'){this.edit('delete');}else if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='z'){e.preventDefault();this.edit('undo');}};
 private cancel(){this.working=undefined;this.start=undefined;this.paint();}
 private anchor(e:PointerEvent):Anchor|undefined {
  if(this.times.length<2)return;const r=this.host.getBoundingClientRect(),x=e.clientX-r.left,y=e.clientY-r.top;
  if(x<0||x>Number(this.chart.timeScale().width())||y<0||y>(Number(this.host.dataset.pricePaneHeight)||this.host.clientHeight)-12)return;
  const logical=this.chart.timeScale().coordinateToLogical(x),price=this.candles.coordinateToPrice(y);
  if(logical==null||price==null||!Number.isFinite(price)||price<=0)return;
  return {time:indexToTime(this.times,Number(logical)),price};
 }
 bind(symbol:string,times:number[]){
  if(symbol!==this.symbol){this.cancel();this.symbol=symbol;this.key='';}this.times=times;
  this.host.closest('.gl-route-chart')?.querySelectorAll<HTMLButtonElement>('[data-route-draw]').forEach(b=>b.onclick=()=>{drawingState(symbol).tool=b.dataset.routeDraw as DrawTool;this.cancel();this.buttons();this.host.focus({preventScroll:true});});
  this.host.closest('.gl-route-chart')?.querySelectorAll<HTMLButtonElement>('[data-route-edit]').forEach(b=>b.onclick=()=>this.edit(b.dataset.routeEdit!));
  this.paint();this.buttons();
 }
 private edit(action:string){const s=drawingState(this.symbol);if(action==='undo')undoDrawing(this.symbol);else if(s.selected&&!s.locked){commitDrawing(this.symbol,s.rows.filter(d=>d.id!==s.selected));s.selected=null;}this.cancel();this.buttons();}
 private buttons(){const root=this.host.closest('.gl-route-chart'),s=drawingState(this.symbol);root?.querySelectorAll<HTMLButtonElement>('[data-route-draw]').forEach(b=>b.setAttribute('aria-pressed',String(b.dataset.routeDraw===s.tool)));const del=root?.querySelector<HTMLButtonElement>('[data-route-edit="delete"]'),undo=root?.querySelector<HTMLButtonElement>('[data-route-edit="undo"]');if(del)del.disabled=!s.selected;if(undo)undo.disabled=!s.undo.length;const help=root?.querySelector('.route-draw-help');if(help){help.textContent=s.error||'ⓘ';help.setAttribute('title',s.error||'Marcas manuales guardadas por activo · arrastra para dibujar · Esc: volver al cursor');}}
 paint(){
  const s=drawingState(this.symbol),w=Number(this.chart.timeScale().width()),h=this.host.clientHeight;
  this.host.classList.toggle('route-cursor-pan',s.tool==='pan');this.svg.style.pointerEvents=s.tool==='pan'?'none':'auto';this.svg.style.cursor=s.tool==='select'?'default':'crosshair';this.svg.style.width=w+'px';this.svg.style.height=Math.max(0,h-28)+'px';
  this.svg.setAttribute('viewBox',`0 0 ${w} ${Math.max(0,h-28)}`);
  const html=s.hidden?'':drawingShapes(this.working??s.rows,s.selected,this.times,t=>this.chart.timeScale().logicalToCoordinate(timeToIndex(this.times,t) as Logical),p=>this.candles.priceToCoordinate(p),w);
  if(this.key!==html){this.key=html;this.svg.innerHTML=html;}
 }
 dispose(){this.host.classList.remove('route-cursor-pan');this.host.removeEventListener('keydown',this.keyboard);this.svg.remove();}
}
