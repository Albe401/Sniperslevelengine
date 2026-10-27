import type {IChartApi,ISeriesApi,UTCTimestamp} from 'lightweight-charts';
import type {ChartSignalCoordinatorReport} from './chart-signal-coordinator';
import {ChartSignalDisplay,signalDisplaySvg,signalKnownBar,type SignalDisplayContext,type SignalDisplayView} from './chart-signal-display';

/** One lightweight SVG layer in the existing chart cadence. No feed, timer or engine evaluation. */
export class ChartSignalDisplayLayer {
 private svg:SVGSVGElement;private status:HTMLDivElement;private view:SignalDisplayView|undefined;private context:SignalDisplayContext|undefined;private common:Readonly<ChartSignalCoordinatorReport>|null|undefined;private enabled=true;private intervalMs=60000;private bars:readonly number[]=[];private html='';
 constructor(private host:HTMLElement,private chart:IChartApi,private series:ISeriesApi<'Candlestick'>,private display:ChartSignalDisplay){
  this.svg=document.createElementNS('http://www.w3.org/2000/svg','svg');this.svg.classList.add('gl-call-put-signals');this.svg.setAttribute('aria-label','Confirmaciones técnicas CALL y PUT');
  this.status=document.createElement('div');this.status.className='gl-call-put-status';this.status.setAttribute('role','status');this.status.setAttribute('aria-live','polite');host.append(this.svg,this.status);
  const close=(event:Event)=>{const target=event.target instanceof Element?event.target.closest<SVGElement>('[data-dismiss-chart-signal]'):null;if(!target||event instanceof KeyboardEvent&&!['Enter',' '].includes(event.key))return;event.preventDefault();event.stopPropagation();if(this.display.dismiss(target.dataset.dismissChartSignal??''))this.refresh();};
  this.svg.addEventListener('click',close);this.svg.addEventListener('keydown',close);this.svg.addEventListener('pointerdown',e=>{if(e.target instanceof Element&&e.target.closest('[data-dismiss-chart-signal]'))e.stopPropagation();});
 }
 update(common:Readonly<ChartSignalCoordinatorReport>|null|undefined,context:SignalDisplayContext,bars:readonly number[],intervalMs:number,enabled:boolean){this.intervalMs=intervalMs;this.common=common;this.context=context;this.bars=bars;this.enabled=enabled;this.refresh();}
 private refresh(){if(!this.context)return;this.view=this.display.read(this.common,this.context,this.enabled);this.paint();}
 paint(){
  if(!this.view||!this.context)return;const v=this.view,now=v.origin==='snapshot'?v.asOfMs:Date.now(),fresh=now<=v.freshUntilMs,signals=fresh?v.signals.filter(s=>now<s.ticket.deadlineMs):[];
  const prefix=v.origin==='snapshot'?'CAPTURA · ':'PRUEBA EN VIVO · ',status=prefix+(!fresh&&v.enabled?'Señales CALL/PUT · esperando datos recientes':v.status);
  if(this.status.textContent!==status)this.status.textContent=status;this.status.title=v.detail;this.status.hidden=!v.enabled;
  this.host.dataset.chartCallPutSignals=v.enabled?'enabled':'disabled';this.host.dataset.chartCallPutCount=String(signals.length);this.host.dataset.chartCallPutSource=String(v.sourcePublishedMs??'');
  const width=this.host.clientWidth,height=this.chart.panes()[0].getHeight();this.svg.setAttribute('width',String(width));this.svg.setAttribute('height',String(height));
  const html=signalDisplaySvg(signals,s=>{const bar=signalKnownBar(this.bars,s.ticket.confirmedAtMs,this.intervalMs);if(bar==null)return null;const x=this.chart.timeScale().timeToCoordinate(bar/1000 as UTCTimestamp),y=this.series.priceToCoordinate(s.ticket.entryReference!.price);return x==null||y==null?null:{x,y,targetY:this.series.priceToCoordinate(s.ticket.target!.price),stopY:this.series.priceToCoordinate(s.ticket.invalidation!.price)};},width,height,v.origin);
  if(this.html!==html){this.svg.innerHTML=html;this.html=html;}
 }
 dispose(){this.svg.remove();this.status.remove();delete this.host.dataset.chartCallPutSignals;delete this.host.dataset.chartCallPutCount;delete this.host.dataset.chartCallPutSource;}
}
