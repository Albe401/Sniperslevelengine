import type {LiveState} from '../live';
import {dualData,type PointSeries} from './model';
import {renderDual,defaultView} from './render';
import './style.css';
/** Mount only when the new module is authorized. Updates come from its owner. */
export function mountGammaDual(host:HTMLElement,symbol:string){
 let state:LiveState|undefined,active=false,interval=2,now=Date.now(),points:PointSeries|undefined,view=defaultView();
 const paint=()=>{host.innerHTML=renderDual(dualData(state,symbol,active,interval,now,points),view);};
 const click=(event:Event)=>{const button=(event.target as Element).closest<HTMLButtonElement>('button');if(!button)return;
  if(button.dataset.gdInterval){interval=Number(button.dataset.gdInterval);view.range=undefined;view.offset=0;}
  switch(button.dataset.gdAction){case 'older':view.offset+=30;break;case 'newer':view.offset=Math.max(0,view.offset-30);break;case 'zoom-in':view.visible=Math.max(10,Math.round(view.visible*.8));break;case 'zoom-out':view.visible=Math.min(2000,Math.round(view.visible*1.25));break;case 'reset':view=defaultView();break;case 'expand':view.expanded=!view.expanded;break;case 'vwap':view.vwap=!view.vwap;break;}paint();
 };
 const wheel=(event:WheelEvent)=>{if(!(event.target as Element).closest('[data-gd-chart]'))return;event.preventDefault();if(event.shiftKey){const data=dualData(state,symbol,active,interval,now,points),values=[...data.bars.map(b=>b.close),...data.rows.map(r=>r.strike)];if(!values.length)return;const range=view.range??[Math.min(...values)*.999,Math.max(...values)*1.001],center=(range[0]+range[1])/2,half=(range[1]-range[0])/2*(event.deltaY>0?1.1:.9);view.range=[Math.max(.001,center-half),center+half];}else view.visible=Math.max(10,Math.min(2000,Math.round(view.visible*(event.deltaY>0?1.1:.9))));paint();};
 host.addEventListener('click',click);host.addEventListener('wheel',wheel,{passive:false});paint();
 return {update(next:LiveState|undefined,connected:boolean,at=Date.now(),nextPoints?:PointSeries){state=next;active=connected;now=at;points=nextPoints;paint();},setSymbol(next:string){symbol=next;state=undefined;points=undefined;view=defaultView();paint();},dispose(){host.removeEventListener('click',click);host.removeEventListener('wheel',wheel);host.replaceChildren();}};
}
