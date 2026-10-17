import {createChart,CandlestickSeries,HistogramSeries,ColorType,CrosshairMode,LineStyle,type IChartApi,type ISeriesApi,type AutoscaleInfo,type Time,type UTCTimestamp} from 'lightweight-charts';
import type {LiveState} from './live';
import type {Candle} from './model';
import type {AuctionReading} from './auction-map-panel';
import {routeReferences,routeHeatCells} from './level-route-reading';
import {levelBandStyle,type ColoredMapLevel} from './level-band-style';
import type {Group,MapSelection} from './two-maps';

export interface TradingMapModel {symbol:string;asof:number;bars:Candle[];groups:Group[];levels:ColoredMapLevel[];selection:MapSelection;auction?:AuctionReading}
const day=(t:number)=>new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'}).format(t);
/** Canonical OHLC only: never synthesize a candle from its close. */
export function tradingMapBars(state:LiveState|undefined,asof:number,minutes=1,hours:'all'|'regular'='all'):Candle[]{
 if(!Number.isInteger(minutes)||minutes<=0)throw new Error('Invalid interval');
 const session=state?.chartHistory?.sessions?.filter(s=>s.date===day(asof)&&Number.isFinite(s.openMs)&&Number.isFinite(s.closeMs)&&s.closeMs>s.openMs);
 if(hours==='regular'&&session?.length!==1)return [];
 const source=new Map((state?.chartHistory?.minutes??[]).map(b=>[b.timestampMs,b]));
 for(const b of Object.values(state?.minutes??{}))source.set(b.timestampMs,b);
 const valid=[...source.values()].filter(b=>Number.isSafeInteger(b.timestampMs)&&b.timestampMs%60000===0&&b.timestampMs<=asof&&day(b.timestampMs)===day(asof)&&[b.open,b.high,b.low,b.close].every(n=>Number.isFinite(n)&&n>0)&&b.high>=Math.max(b.open,b.close)&&b.low<=Math.min(b.open,b.close)&&b.low<=b.high).sort((a,b)=>a.timestampMs-b.timestampMs);
 const buckets=new Map<number,Candle>();
 for(const b of valid){if(hours==='regular'&&(b.timestampMs<session![0].openMs||b.timestampMs>=session![0].closeMs))continue;const t=Math.floor(b.timestampMs/(minutes*60000))*minutes*60000,old=buckets.get(t);const volume=Number.isFinite(b.volume)&&b.volume>=0?b.volume:NaN;
  if(old){old.high=Math.max(old.high,b.high);old.low=Math.min(old.low,b.low);old.close=b.close;old.volume+=volume;}
  else buckets.set(t,{...b,timestampMs:t,volume});
 }
 return [...buckets.values()];
}
/** Preserve gaps inside observed data without appending empty bars up to wall-clock time. */
export function tradingMapTimeline(bars:Candle[],asof:number,minutes=1,groups:Group[]=[]){
 if(!Number.isInteger(minutes)||minutes<=0)throw new Error('Invalid interval');
 const step=minutes*60000,limit=Math.floor(asof/step)*step;
 const observed=[...bars.map(b=>b.timestampMs),...groups.map(g=>Math.floor(g.startMs/step)*step)].filter(t=>Number.isFinite(t)&&t<=limit);
 const first=observed.length?Math.min(...observed):limit-60*60000,last=observed.length?Math.max(...observed):limit;
 const rows=new Map(bars.map(b=>[b.timestampMs,b]));
 const data=[];
 for(let t=first;t<=last;t+=step){const b=rows.get(t),time=t/1000 as UTCTimestamp;data.push(b?{time,open:b.open,high:b.high,low:b.low,close:b.close}:{time});}
 return {first,last,data};
}
let latest:TradingMapModel|undefined;
export function setTradingMapModel(model:TradingMapModel){latest=model;}
let owner:TradingMapChart|undefined;
const viewports=new Map<string,{from:number;to:number;first:number;follow:boolean}>();
export function heatmapMinuteRect(timestamp:number,minutes:number,anchor:number,barWidth:number){
 const bucket=Math.floor(timestamp/(minutes*60000))*minutes*60000;
 return {left:anchor-barWidth/2+(timestamp-bucket)/60000*barWidth/minutes,width:barWidth/minutes};
}
export function bindTradingMap(pick:(strike:number,minute:number,side:'call'|'put')=>void){
 const node=document.querySelector<HTMLElement>('#snipers-trading-map');if(!node||!latest)return;
 if(owner?.node!==node){owner?.dispose();owner=new TradingMapChart(node,pick);}
 owner.update(latest);
}
export function tradingMapAction(action:string){owner?.action(action);}
class TradingMapChart {
 chart:IChartApi; candles:ISeriesApi<'Candlestick'>;volume:ISeriesApi<'Histogram'>;
 canvas:HTMLCanvasElement;model?:TradingMapModel;key='';signature='';frame=0;lines:ReturnType<ISeriesApi<'Candlestick'>['createPriceLine']>[]=[];
 hits:{x:number;y:number;w:number;h:number;g:Group}[]=[];follow=true;lastBar=0;first=0;
 constructor(public node:HTMLElement,pick:(strike:number,minute:number,side:'call'|'put')=>void){
  this.chart=createChart(node,{autoSize:true,layout:{background:{type:ColorType.Solid,color:'#0b111b'},textColor:'#8392a6',fontFamily:'Inter, sans-serif',fontSize:11,attributionLogo:true,panes:{enableResize:true,separatorColor:'#202a39',separatorHoverColor:'#53667f'}},grid:{vertLines:{color:'#151e2b'},horzLines:{color:'#151e2b'}},crosshair:{mode:CrosshairMode.Normal,vertLine:{color:'#6e8099',labelBackgroundColor:'#34435a'},horzLine:{color:'#6e8099',labelBackgroundColor:'#34435a'}},rightPriceScale:{borderColor:'#263144',minimumWidth:78,scaleMargins:{top:.12,bottom:.12}},timeScale:{borderColor:'#263144',timeVisible:true,secondsVisible:false,rightOffset:5,barSpacing:10,tickMarkFormatter:(t:Time)=>new Date(Number(t)*1000).toLocaleTimeString('es',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit'})},localization:{locale:'en-US',timeFormatter:(t:Time)=>new Date(Number(t)*1000).toLocaleString('es',{timeZone:'America/New_York'})},handleScroll:{mouseWheel:true,pressedMouseMove:true,horzTouchDrag:true,vertTouchDrag:true},handleScale:{mouseWheel:true,pinch:true,axisPressedMouseMove:{time:true,price:true},axisDoubleClickReset:{time:true,price:true}}});
  this.candles=this.chart.addSeries(CandlestickSeries,{upColor:'#26bda6',downColor:'#ed6574',wickUpColor:'#26bda6',wickDownColor:'#ed6574',borderVisible:false,priceLineColor:'#a3afbf'});
  this.volume=this.chart.addSeries(HistogramSeries,{priceFormat:{type:'volume'},priceLineVisible:false,lastValueVisible:false},1);
  this.chart.panes()[0].setStretchFactor(7);this.chart.panes()[1].setStretchFactor(1);
  this.canvas=document.createElement('canvas');this.canvas.className='tm-trading-overlay';this.canvas.setAttribute('aria-hidden','true');node.appendChild(this.canvas);
  this.chart.subscribeClick(p=>{if(!p.point)return;const hit=[...this.hits].reverse().find(h=>p.point!.x>=h.x&&p.point!.x<=h.x+h.w&&p.point!.y>=h.y&&p.point!.y<=h.y+h.h);if(hit)pick(hit.g.strike,hit.g.startMs,hit.g.kind as 'call'|'put');});
  this.chart.subscribeCrosshairMove(p=>{const label=node.parentElement?.querySelector<HTMLElement>('[data-map-ohlc]');if(!label)return;const row=p.seriesData.get(this.candles);label.textContent=row&&'open'in row?`O ${row.open.toFixed(2)}   H ${row.high.toFixed(2)}   L ${row.low.toFixed(2)}   C ${row.close.toFixed(2)}`:'Arrastra para desplazar · rueda para zoom · doble clic en eje para ajustar';});
  this.node.addEventListener('pointerdown',()=>{this.follow=false;});this.node.addEventListener('wheel',()=>{this.follow=false;},{passive:true});
  const paint=()=>{if(!this.node.isConnected){this.dispose();if(owner===this)owner=undefined;return;}const range=this.chart.timeScale().getVisibleLogicalRange(),height=this.chart.panes()[0].getHeight();const probe=this.candles.coordinateToPrice(0),signature=JSON.stringify([range,node.clientWidth,node.clientHeight,height,probe,this.candles.coordinateToPrice(height)]);if(signature!==this.signature){this.signature=signature;this.draw();}this.frame=requestAnimationFrame(paint);};this.frame=requestAnimationFrame(paint);
 }
 update(m:TradingMapModel){
  const key=`${m.symbol}:${m.selection.barMinutes??1}:${m.selection.extendedHours?'extended':'regular'}`,changed=key!==this.key;this.key=key;this.model=m;
  const step=(m.selection.barMinutes??1)*60000;
  const {first,last:asof,data}=tradingMapTimeline(m.bars,m.asof,m.selection.barMinutes??1,m.groups);
  const heat=m.selection.view==='flow';
  this.candles.applyOptions({autoscaleInfoProvider:(base:()=>AutoscaleInfo|null)=>{
   const info=base();if(!heat){if(!info?.priceRange)return info;const span=Math.max(info.priceRange.maxValue-info.priceRange.minValue,.2),prices=m.levels.map(l=>l.price).filter(p=>p>=info.priceRange!.minValue-span&&p<=info.priceRange!.maxValue+span);const low=Math.min(...prices,info.priceRange.minValue),high=Math.max(...prices,info.priceRange.maxValue),pad=Math.max((high-low)*.06,.1);return {priceRange:{minValue:low-pad,maxValue:high+pad}};}if(!m.groups.length)return info;
   const center=m.selection.strike??m.bars.at(-1)?.close??m.groups[0].strike;
   const strikes=[...new Set(m.groups.map(g=>g.strike))].sort((a,b)=>Math.abs(a-center)-Math.abs(b-center)).slice(0,25);
   const low=Math.min(...strikes,info?.priceRange?.minValue??Infinity),high=Math.max(...strikes,info?.priceRange?.maxValue??-Infinity),pad=Math.max((high-low)*.04,.01);
   return {priceRange:{minValue:low-pad,maxValue:high+pad}};
  }});
  this.candles.applyOptions(heat?{upColor:'#6e849288',downColor:'#6e849244',wickUpColor:'#8ca1b5',wickDownColor:'#708497',borderVisible:true,borderUpColor:'#a4b5c5',borderDownColor:'#718595'}:{upColor:'#26bda6',downColor:'#ed6574',wickUpColor:'#26bda6',wickDownColor:'#ed6574',borderVisible:false});
  this.candles.setData(data);this.volume.setData(m.bars.filter(b=>Number.isFinite(b.volume)).map(b=>({time:b.timestampMs/1000 as UTCTimestamp,value:b.volume,color:b.close>=b.open?'#1b756b80':'#99465680'})));
  for(const line of this.lines)this.candles.removePriceLine(line);this.lines=[];
  for(const l of m.levels)this.lines.push(this.candles.createPriceLine({price:l.price,color:/Main Magnet/.test(l.name)?'#3dd7bb':/Wall/.test(l.name)?'#a687d8':'#c87864',lineWidth:1,lineStyle:LineStyle.Solid,lineVisible:heat,axisLabelVisible:true,title:heat?l.name.replace('Main Magnet','IMÁN PRINCIPAL').replace('Call Wall','CALL WALL').replace('Put Wall','PUT WALL'):''}));
  if(changed){const remembered=viewports.get(key);this.follow=remembered?.follow??true;const shift=remembered?(remembered.first-first)/step:0;this.chart.timeScale().setVisibleLogicalRange(remembered?{from:remembered.from+shift,to:remembered.to+shift}:{from:Math.max(0,data.length-(heat?36:75)/(m.selection.barMinutes??1)),to:data.length+(heat?3:20)});}
  else if(this.follow&&asof!==this.lastBar)this.chart.timeScale().scrollToRealTime();
  this.first=first;this.lastBar=asof;this.draw();
 }
 draw(){
  const m=this.model;if(!m)return;const w=this.node.clientWidth,h=this.chart.panes()[0].getHeight(),ratio=window.devicePixelRatio||1;
  this.canvas.width=Math.round(w*ratio);this.canvas.height=Math.round(h*ratio);this.canvas.style.width=`${w}px`;this.canvas.style.height=`${h}px`;const ctx=this.canvas.getContext('2d');if(!ctx)return;ctx.scale(ratio,ratio);this.hits=[];
  if(m.selection.view==='levels'){this.drawRoute(ctx,m,w,h);return;}
  const strikes=[...new Set(m.groups.map(g=>g.strike))].sort((a,b)=>a-b),step=strikes.length>1?Math.min(...strikes.slice(1).map((p,i)=>p-strikes[i])):1;
  const max=Math.max(0,...m.groups.map(g=>g[m.selection.metric]??0));
  const plotW=w-this.chart.priceScale('right').width();ctx.save();ctx.beginPath();ctx.rect(0,0,plotW,h);ctx.clip();
  for(const strike of strikes){const y=this.candles.priceToCoordinate(strike);if(y==null)continue;ctx.strokeStyle='#25374b';ctx.setLineDash([2,5]);ctx.beginPath();ctx.moveTo(0,y);ctx.lineTo(plotW,y);ctx.stroke();ctx.setLineDash([]);}
  for(const g of m.groups){if(m.selection.heatSide&&m.selection.heatSide!=='both'&&m.selection.heatSide!==g.kind)continue;const v=g[m.selection.metric];if(v==null||!max)continue;
   const minutes=m.selection.barMinutes??1,bucket=Math.floor(g.startMs/(minutes*60000))*minutes*60000;
   const x=this.chart.timeScale().timeToCoordinate(bucket/1000 as UTCTimestamp),next=this.chart.timeScale().timeToCoordinate((bucket+minutes*60000)/1000 as UTCTimestamp),y=this.candles.priceToCoordinate(g.strike),y2=this.candles.priceToCoordinate(g.strike+step*.18);
   if(x==null||y==null||y2==null)continue;const rect=heatmapMinuteRect(g.startMs,minutes,x,(next??(x+this.chart.timeScale().options().barSpacing))-x),cw=Math.max(.8,rect.width),bh=Math.max(3,Math.min(24,Math.abs(y2-y))),top=g.kind==='call'?y-bh:y+1,left=rect.left;
   if(y+bh<0||y-bh>h)continue;const intensity=v/max;
   ctx.fillStyle=g.kind==='call'?`rgba(${Math.round(22+145*intensity)},${Math.round(120+130*intensity)},${Math.round(135+110*intensity)},${.25+.7*intensity})`:`rgba(${Math.round(155+100*intensity)},${Math.round(80+151*intensity)},${Math.round(25+100*intensity)},${.25+.7*intensity})`;ctx.fillRect(left,top,cw-.6,bh);
   ctx.fillStyle=g.kind==='call'?'#47cbb9':'#e5b366';ctx.fillRect(left,top,cw-.6,1);
   if(g.strike===m.selection.strike&&g.startMs===m.selection.minute&&g.kind===m.selection.side){ctx.strokeStyle='#eef2f8';ctx.strokeRect(left,top,cw-.6,bh);}
   this.hits.push({x:left,y:top,w:cw,h:bh,g});
  }ctx.restore();
 }
 drawRoute(ctx:CanvasRenderingContext2D,m:TradingMapModel,w:number,h:number){
  const plotW=w-this.chart.priceScale('right').width();ctx.save();ctx.beginPath();ctx.rect(0,0,plotW,h);ctx.clip();
  this.drawLevelBands(ctx,m,plotW,false);
  // Actual underlying profile only; option strike volume never substitutes for it.
  const bins=m.auction?.bins??[],peak=Math.max(0,...bins.map(b=>b.volume));
  if(peak>0){for(const b of bins){const top=this.candles.priceToCoordinate(b.high),bottom=this.candles.priceToCoordinate(b.low);if(top==null||bottom==null)continue;ctx.fillStyle='#6e8ca144';ctx.fillRect(0,top,Math.max(1,b.volume/peak*plotW*.105),Math.max(1,bottom-top));}ctx.font='10px Inter, sans-serif';ctx.fillStyle='#9ab4cf';ctx.fillText('AMT · SUBYACENTE'+(m.auction?.provisional?' · PARCIAL':''),12,19);}
  // Neutral amber measures received executed volume; never buy/sell intent.
  const heatCells=routeHeatCells(m.groups,m.asof,m.selection.metric);
  const maximum=Math.max(0,...heatCells.map(x=>x.value));
  const minutes=m.selection.barMinutes??1;
  for(const {g,value} of heatCells){
   if(value<=0||!maximum)continue;const bucket=Math.floor(g.startMs/(minutes*60000))*minutes*60000;
   const x=this.chart.timeScale().timeToCoordinate(bucket/1000 as UTCTimestamp),nx=this.chart.timeScale().timeToCoordinate((bucket+minutes*60000)/1000 as UTCTimestamp),y=this.candles.priceToCoordinate(g.strike);if(x==null||y==null)continue;
   const rect=heatmapMinuteRect(g.startMs,minutes,x,(nx??(x+this.chart.timeScale().options().barSpacing))-x),alpha=value/maximum;
   const gradient=ctx.createLinearGradient(0,y-11,0,y+11);gradient.addColorStop(0,'rgba(255,147,33,0)');gradient.addColorStop(.5,'rgba(255,166,47,'+(.12+.43*alpha)+')');gradient.addColorStop(1,'rgba(255,147,33,0)');ctx.fillStyle=gradient;ctx.fillRect(rect.left,y-11,Math.max(1,rect.width+.2),22);ctx.fillStyle='rgba(255,190,68,'+(.12+.45*alpha)+')';ctx.fillRect(rect.left,y-1,Math.max(1,rect.width+.2),2);
   this.hits.push({x:rect.left,y:y-11,w:Math.max(1,rect.width),h:22,g});
  }
  this.drawLevelBands(ctx,m,plotW,true);
  const last=m.bars.at(-1),x=last?this.chart.timeScale().timeToCoordinate(last.timestampMs/1000 as UTCTimestamp):null;
  const refs=routeReferences(m.levels,last?.close??null);
  if(last&&x!=null&&x<plotW-70){const startX=Math.max(x+12,plotW*.74),targetX=plotW-30;ctx.fillStyle='#94b5d509';ctx.fillRect(startX,0,plotW-startX,h);ctx.fillStyle='#8ea8c3';ctx.font='10px Inter, sans-serif';ctx.fillText('REFERENCIAS · RUTA PENDIENTE',Math.max(startX+8,plotW-235),20);
   const y=this.candles.priceToCoordinate(last.close);
   if(y!=null)for(const [target,label] of [[refs.upper,'Superior'],[refs.lower,'Inferior']] as const){if(target==null)continue;const ty=this.candles.priceToCoordinate(target);if(ty==null)continue;ctx.strokeStyle='#8bacc6a0';ctx.lineWidth=1.5;ctx.setLineDash([3,6]);ctx.beginPath();ctx.moveTo(x+6,y);ctx.lineTo(targetX,ty);ctx.stroke();ctx.setLineDash([]);const angle=Math.atan2(ty-y,targetX-x-6);ctx.beginPath();ctx.moveTo(targetX-7*Math.cos(angle-.5),ty-7*Math.sin(angle-.5));ctx.lineTo(targetX,ty);ctx.lineTo(targetX-7*Math.cos(angle+.5),ty-7*Math.sin(angle+.5));ctx.stroke();ctx.font='10px Inter, sans-serif';ctx.fillStyle='#a8bfd2';ctx.fillText(label,Math.max(startX+8,targetX-65),ty+(ty<y?18:-10));}
  }
  ctx.restore();
 }
 drawLevelBands(ctx:CanvasRenderingContext2D,m:TradingMapModel,width:number,labels:boolean){
  ctx.font='600 11px Inter, sans-serif';
  for(const level of m.levels){const y=this.candles.priceToCoordinate(level.price);if(y==null)continue;const style=levelBandStyle(level),top=y-style.height/2;
   if(!labels){ctx.fillStyle=style.fill;ctx.fillRect(0,top,width,style.height);ctx.strokeStyle=style.edge;ctx.lineWidth=1;ctx.strokeRect(.5,top+.5,width-1,style.height-1);}
   else{const labelWidth=Math.min(width-12,ctx.measureText(style.label).width+20),left=width-labelWidth-6;ctx.fillStyle=style.fill;ctx.fillRect(left,top,labelWidth,style.height);ctx.strokeStyle=style.edge;ctx.strokeRect(left+.5,top+.5,labelWidth-1,style.height-1);ctx.fillStyle='#e0eaf6';ctx.textBaseline='middle';ctx.fillText(style.label,left+10,y,labelWidth-18);ctx.textBaseline='alphabetic';}
  }
 }
 action(action:string){if(action==='fit'){this.chart.timeScale().fitContent();this.candles.priceScale().applyOptions({autoScale:true});this.follow=false;}if(action==='follow'){this.follow=true;this.chart.timeScale().scrollToRealTime();this.candles.priceScale().applyOptions({autoScale:true});}this.draw();}
 dispose(){const range=this.chart.timeScale().getVisibleLogicalRange();if(range&&this.key){if(viewports.size>20)viewports.clear();viewports.set(this.key,{...range,first:this.first,follow:this.follow});}cancelAnimationFrame(this.frame);this.chart.remove();}
}

