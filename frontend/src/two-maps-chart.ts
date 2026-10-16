import {createChart,CandlestickSeries,HistogramSeries,ColorType,CrosshairMode,LineStyle,type IChartApi,type ISeriesApi,type AutoscaleInfo,type Time,type UTCTimestamp} from 'lightweight-charts';
import type {LiveState} from './live';
import type {Candle} from './model';
import type {Group,MapSelection} from './two-maps';

export interface TradingMapModel {symbol:string;asof:number;bars:Candle[];groups:Group[];levels:{price:number;name:string}[];selection:MapSelection}
const day=(t:number)=>new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'}).format(t);
/** Canonical OHLC only: never synthesize a candle from its close. */
export function tradingMapBars(state:LiveState|undefined,asof:number,minutes=1):Candle[]{
 const source=new Map((state?.chartHistory?.minutes??[]).map(b=>[b.timestampMs,b]));
 for(const b of Object.values(state?.minutes??{}))source.set(b.timestampMs,b);
 const valid=[...source.values()].filter(b=>Number.isSafeInteger(b.timestampMs)&&b.timestampMs%60000===0&&b.timestampMs<=asof&&b.timestampMs>=asof-6*3600000&&day(b.timestampMs)===day(asof)&&[b.open,b.high,b.low,b.close].every(n=>Number.isFinite(n)&&n>0)&&b.high>=Math.max(b.open,b.close)&&b.low<=Math.min(b.open,b.close)&&b.low<=b.high).sort((a,b)=>a.timestampMs-b.timestampMs);
 const buckets=new Map<number,Candle>();
 for(const b of valid){const t=Math.floor(b.timestampMs/(minutes*60000))*minutes*60000,old=buckets.get(t);const volume=Number.isFinite(b.volume)&&b.volume>=0?b.volume:NaN;
  if(old){old.high=Math.max(old.high,b.high);old.low=Math.min(old.low,b.low);old.close=b.close;old.volume+=volume;}
  else buckets.set(t,{...b,timestampMs:t,volume});
 }
 return [...buckets.values()];
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
  const key=`${m.symbol}:${m.selection.barMinutes??1}`,changed=key!==this.key;this.key=key;this.model=m;
  const step=(m.selection.barMinutes??1)*60000,asof=Math.floor(m.asof/step)*step,first=m.bars[0]?.timestampMs??asof-60*60000;
  const rows=new Map(m.bars.map(b=>[b.timestampMs,b]));const data=[];
  for(let t=first;t<=asof;t+=step){const b=rows.get(t),time=t/1000 as UTCTimestamp;data.push(b?{time,open:b.open,high:b.high,low:b.low,close:b.close}:{time});}
  const heat=m.selection.view==='flow';
  this.candles.applyOptions({autoscaleInfoProvider:(base:()=>AutoscaleInfo|null)=>{
   const info=base();if(!heat||!m.groups.length)return info;
   const center=m.selection.strike??m.bars.at(-1)?.close??m.groups[0].strike;
   const strikes=[...new Set(m.groups.map(g=>g.strike))].sort((a,b)=>Math.abs(a-center)-Math.abs(b-center)).slice(0,25);
   const low=Math.min(...strikes,info?.priceRange?.minValue??Infinity),high=Math.max(...strikes,info?.priceRange?.maxValue??-Infinity),pad=Math.max((high-low)*.04,.01);
   return {priceRange:{minValue:low-pad,maxValue:high+pad}};
  }});
  this.candles.applyOptions(heat?{upColor:'#6e849288',downColor:'#6e849244',wickUpColor:'#8ca1b5',wickDownColor:'#708497',borderVisible:true,borderUpColor:'#a4b5c5',borderDownColor:'#718595'}:{upColor:'#26bda6',downColor:'#ed6574',wickUpColor:'#26bda6',wickDownColor:'#ed6574',borderVisible:false});
  this.candles.setData(data);this.volume.setData(m.bars.filter(b=>Number.isFinite(b.volume)).map(b=>({time:b.timestampMs/1000 as UTCTimestamp,value:b.volume,color:b.close>=b.open?'#1b756b80':'#99465680'})));
  for(const line of this.lines)this.candles.removePriceLine(line);this.lines=[];
  for(const l of m.levels)this.lines.push(this.candles.createPriceLine({price:l.price,color:l.name==='Call Wall'?'#e6b960':l.name==='Put Wall'?'#6ba4ef':'#b89ae8',lineWidth:1,lineStyle:LineStyle.Dashed,axisLabelVisible:true,title:l.name}));
  if(changed){const remembered=viewports.get(key);this.follow=remembered?.follow??true;const shift=remembered?(remembered.first-first)/step:0;this.chart.timeScale().setVisibleLogicalRange(remembered?{from:remembered.from+shift,to:remembered.to+shift}:{from:Math.max(0,data.length-(heat?36:75)/(m.selection.barMinutes??1)),to:data.length+3});}
  else if(this.follow&&asof!==this.lastBar)this.chart.timeScale().scrollToRealTime();
  this.first=first;this.lastBar=asof;this.draw();
 }
 draw(){
  const m=this.model;if(!m)return;const w=this.node.clientWidth,h=this.chart.panes()[0].getHeight(),ratio=window.devicePixelRatio||1;
  this.canvas.width=Math.round(w*ratio);this.canvas.height=Math.round(h*ratio);this.canvas.style.width=`${w}px`;this.canvas.style.height=`${h}px`;const ctx=this.canvas.getContext('2d');if(!ctx)return;ctx.scale(ratio,ratio);this.hits=[];
  if(m.selection.view!=='flow')return;
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
 action(action:string){if(action==='fit'){this.chart.timeScale().fitContent();this.candles.priceScale().applyOptions({autoScale:true});this.follow=false;}if(action==='follow'){this.follow=true;this.chart.timeScale().scrollToRealTime();this.candles.priceScale().applyOptions({autoScale:true});}this.draw();}
 dispose(){const range=this.chart.timeScale().getVisibleLogicalRange();if(range&&this.key){if(viewports.size>20)viewports.clear();viewports.set(this.key,{...range,first:this.first,follow:this.follow});}cancelAnimationFrame(this.frame);this.chart.remove();}
}
