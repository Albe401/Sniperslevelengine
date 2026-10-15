import {it,expect,vi,afterEach} from 'vitest';
import {bindDrawingTools,drawingId,validateDrawings,timeToIndex,indexToTime,commitDrawing,drawingState,undoDrawing,drawingsSvg,type Drawing} from './chart-drawings';
const row:Drawing={id:'a',kind:'block',a:{time:1000,price:100},b:{time:3000,price:105},color:'#24dfff'};
it('rejects corrupt persisted marks, duplicate IDs, nonfinite prices and injected colors',()=>{
 expect(validateDrawings([row,{...row},{...row,id:'b',a:{time:1,price:NaN}},{...row,id:'c',color:'" onload="bad'}])).toEqual([row]);
 expect(validateDrawings({rows:[row]})).toEqual([]);
});
it('preserves timestamp anchors through missing candles, zoom and outside the viewport',()=>{
 const times=[1000,2000,8000,9000];
 for(const t of [500,1000,2500,6000,8000,10000])expect(indexToTime(times,timeToIndex(times,t))).toBeCloseTo(t);
 expect(timeToIndex(times,8000)).toBe(2);expect(timeToIndex(times.slice(1),8000)).toBe(1);
});
it('isolates symbols and supports undo redo without losing persisted marks',()=>{
 const memory=new Map<string,string>();vi.stubGlobal('localStorage',{getItem:(k:string)=>memory.get(k)??null,setItem:(k:string,v:string)=>memory.set(k,v)});
 commitDrawing('QA_A',[row]);commitDrawing('QA_A',[{...row,b:{time:4000,price:109}}]);
 expect(drawingState('QA_B').rows).toEqual([]);undoDrawing('QA_A');expect(drawingState('QA_A').rows[0].b.price).toBe(105);
 undoDrawing('QA_A',true);expect(drawingState('QA_A').rows[0].b.price).toBe(109);
 expect(JSON.parse(memory.get('snipers.drawings.v1.QA_A')!)[0].b.price).toBe(109);vi.unstubAllGlobals();
});
it('reports failed saves and retains the drawing in memory',()=>{
 vi.stubGlobal('localStorage',{getItem:()=>null,setItem:()=>{throw Error('quota');}});
 commitDrawing('QA_QUOTA',[row]);expect(drawingState('QA_QUOTA').rows).toHaveLength(1);expect(drawingState('QA_QUOTA').error).toContain('memoria');vi.unstubAllGlobals();
});
it('renders manual blocks with price/time anchors and selectable handles',()=>{
 const html=drawingsSvg([row],'a',{left:0,end:500,candleEnd:400,top:0,floor:200,min:90,max:110,x:i=>i*10,y:p=>110-p},[1000,2000,3000]);
 expect(html).toContain('ORDER BLOCK · MANUAL');expect(html).toContain('data-draw-handle="a"');expect(html).toContain('height="5"');expect(html).not.toContain('NaN');
});

afterEach(()=>vi.unstubAllGlobals());
it('creates unique IDs without randomUUID in embedded webviews',()=>{
 vi.stubGlobal('crypto',{});const ids=Array.from({length:100},()=>drawingId());
 expect(new Set(ids).size).toBe(100);expect(ids.every(id=>id.length<=80)).toBe(true);
});
it('horizontal lines have a generous hit target and only one effective handle',()=>{
 const html=drawingsSvg([{...row,kind:'horizontal'}],'a',{left:0,end:500,candleEnd:400,top:0,floor:200,min:90,max:110,x:i=>i*10,y:p=>110-p},[1000,2000,3000]);
 expect(html).toContain('stroke-width="16"');expect(html).toContain('data-draw-handle="a"');expect(html).not.toContain('data-draw-handle="b"');
});
function gestureFixture(symbol:string){
 vi.stubGlobal('localStorage',{getItem:()=>null,setItem:vi.fn()});
 vi.stubGlobal('crypto',{});
 vi.stubGlobal('DOMPoint',class {constructor(public x=0,public y=0){}matrixTransform(){return this;}});
 const frames=new Map<number,FrameRequestCallback>();let sequence=0;
 vi.stubGlobal('requestAnimationFrame',(cb:FrameRequestCallback)=>{frames.set(++sequence,cb);return sequence;});
 vi.stubGlobal('cancelAnimationFrame',(id:number)=>frames.delete(id));
 const layer={innerHTML:''};
 const svg:any={dataset:{times:'1000,2000,3000',left:'0',end:'400',candleEnd:'300',top:'0',floor:'200',priceMin:'90',priceMax:'110',step:'100',viewStart:'0'},style:{},focus:vi.fn(),setPointerCapture:vi.fn(),getScreenCTM:()=>({inverse:()=>({})}),querySelector:(q:string)=>q==='[data-user-drawings]'?layer:null,append:vi.fn()};
 vi.stubGlobal('document',{querySelector:()=>({querySelectorAll:()=>[],querySelector:(q:string)=>q==='[data-gex-svg]'?svg:null})});
 const hooks={busy:vi.fn(),refresh:vi.fn(),pan:vi.fn(),scale:vi.fn(),zoom:vi.fn(),height:vi.fn()};
 const event=(x:number,y:number)=>({button:0,clientX:x,clientY:y,pointerId:1,preventDefault:vi.fn(),target:{closest:()=>null,getAttribute:()=>null}});
 return {svg,hooks,event,bind:()=>bindDrawingTools(symbol,hooks),frame:()=>{for(const [id,cb] of [...frames]){frames.delete(id);cb(0);}}};
}
it('places HLine on a single click and always releases the live update guard',()=>{
 const f=gestureFixture('QA_HLINE_CLICK');drawingState('QA_HLINE_CLICK').tool='horizontal';f.bind();
 f.svg.onpointerdown(f.event(150,80));f.svg.onpointerup(f.event(150,80));
 expect(drawingState('QA_HLINE_CLICK').rows).toHaveLength(1);
 expect(drawingState('QA_HLINE_CLICK').rows[0].a.price).toBe(102);
 expect(f.hooks.busy).toHaveBeenLastCalledWith(false);expect(f.hooks.refresh).toHaveBeenCalled();
});
it('pans continuously before release, preserves fractional bars and does not apply the movement twice',()=>{
 const f=gestureFixture('QA_PAN_CONTINUOUS');f.bind();
 f.svg.onpointerdown(f.event(150,80));f.svg.onpointermove(f.event(125,90));f.frame();
 expect(f.hooks.pan).toHaveBeenLastCalledWith(.25,1);expect(f.hooks.refresh).not.toHaveBeenCalled();
 f.svg.onpointermove(f.event(100,100));f.svg.onpointerup(f.event(100,100));
 expect(f.hooks.pan.mock.calls).toEqual([[.25,1],[.25,1]]);
 expect(f.hooks.busy).toHaveBeenLastCalledWith(false);
});
it('pointer cancellation discards an unfinished drawing and unblocks live updates',()=>{
 const f=gestureFixture('QA_CANCEL');drawingState('QA_CANCEL').tool='block';f.bind();
 f.svg.onpointerdown(f.event(150,80));f.svg.onpointermove(f.event(200,100));f.svg.onpointercancel();
 expect(drawingState('QA_CANCEL').rows).toEqual([]);expect(f.hooks.busy).toHaveBeenLastCalledWith(false);
});
