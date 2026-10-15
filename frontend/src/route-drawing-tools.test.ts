import {expect,it,vi} from 'vitest';
import {RouteDrawingTools,drawingShapes,measurement,routeDrawingToolbar} from './route-drawing-tools';
import {commitDrawing,drawingState,undoDrawing,validateDrawings,type Drawing} from './chart-drawings';
const d:Drawing={id:'measure',kind:'measure',a:{time:1000,price:100},b:{time:8000,price:105},color:'#24dfff'};
const times=[1000,2000,8000,9000];
it('returns from a drawing tool to chart movement by button or Escape without deleting marks',()=>{
 const svg={style:{pointerEvents:''},classList:{add:vi.fn()},setAttribute:vi.fn(),innerHTML:'',remove:vi.fn(),setPointerCapture:vi.fn()};
 const buttons=['pan','trend'].map(tool=>({dataset:{routeDraw:tool},setAttribute:vi.fn(),onclick:undefined as undefined|(()=>void)}));
 const root={querySelectorAll:(q:string)=>q==='[data-route-draw]'?buttons:[],querySelector:()=>null};
 const keyboard=new Map<string,(e:KeyboardEvent)=>void>();
 const host={append:vi.fn(),closest:()=>root,clientHeight:400,focus:vi.fn(),classList:{toggle:vi.fn(),remove:vi.fn()},addEventListener:(k:string,f:(e:KeyboardEvent)=>void)=>keyboard.set(k,f),removeEventListener:vi.fn()};
 const chart={timeScale:()=>({width:()=>700,logicalToCoordinate:(n:number)=>n*20})},candles={priceToCoordinate:(p:number)=>p};
 vi.stubGlobal('document',{createElementNS:()=>svg});vi.stubGlobal('localStorage',{getItem:()=>null,setItem:vi.fn()});
 try{
  commitDrawing('CURSOR_TEST',[d]);const tools=new RouteDrawingTools(host as unknown as HTMLElement,chart as never,candles as never);tools.bind('CURSOR_TEST',times);
  expect(routeDrawingToolbar('CURSOR_TEST')).toContain('Cursor / Mover');
  buttons[1].onclick!();expect(svg.style.pointerEvents).toBe('auto');expect(host.focus).toHaveBeenCalledWith({preventScroll:true});
  buttons[0].onclick!();expect(svg.style.pointerEvents).toBe('none');expect(drawingState('CURSOR_TEST').tool).toBe('pan');
  buttons[1].onclick!();keyboard.get('keydown')!({key:'Escape'} as KeyboardEvent);expect(svg.style.pointerEvents).toBe('none');expect(drawingState('CURSOR_TEST').rows).toEqual([d]);
  expect(host.classList.toggle).toHaveBeenLastCalledWith('route-cursor-pan',true);tools.dispose();expect(host.classList.remove).toHaveBeenCalledWith('route-cursor-pan');
 }finally{vi.unstubAllGlobals();}
});
it('measures real price change and actual candle distance across session gaps',()=>{
 expect(measurement(d,times)).toBe('+$5.00 · 5.00% · 2.0 velas');
 expect(measurement({...d,a:d.b,b:d.a},times)).toBe('$-5.00 · -4.76% · 2.0 velas');
});
it('retains measurements by symbol and allows undo and removal',()=>{
 vi.stubGlobal('localStorage',{getItem:()=>null,setItem:vi.fn()});
 commitDrawing('TOOLS_A',[d]);expect(drawingState('TOOLS_B').rows).toEqual([]);
 expect(validateDrawings([d])).toEqual([d]);commitDrawing('TOOLS_A',[]);undoDrawing('TOOLS_A');
 expect(drawingState('TOOLS_A').rows).toEqual([d]);
 expect(routeDrawingToolbar('TOOLS_A')).toContain('data-route-draw="measure"');vi.unstubAllGlobals();
});
it('reprojects timestamp and price anchors after zoom without changing persisted data',()=>{
 const a=drawingShapes([d],null,times,t=>t/100,p=>200-p,400);
 const b=drawingShapes([d],'measure',times,t=>t/50,p=>400-2*p,800);
 expect(a).toContain('x="10"');expect(b).toContain('x="20"');expect(b).toContain('stroke-width="3"');
 expect(a).toContain('2.0 velas');expect(d.a.price).toBe(100);
 expect(drawingShapes([d],null,times,()=>null,p=>p,400)).toBe('');
});
it('renders the maximum saved marks within a small update budget',()=>{
 const rows=Array.from({length:200},(_,i)=>({...d,id:String(i)}));const start=performance.now();
 for(let i=0;i<100;i++)drawingShapes(rows,null,times,t=>t/100,p=>p,800);
 const average=(performance.now()-start)/100;console.info(`200 drawings: ${average.toFixed(3)} ms/update`);expect(average).toBeLessThan(20);
});
