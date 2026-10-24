import type {LiveState} from './live';
import {ownDominantHistory,ownDominantMarks,type OwnGammaObservation} from './own-dominant-history';
import {ownHeatDominants} from './own-heat-dominants';
import {paintDominantMarks,type DominantMark} from './dominant-mark-painter';
import {escapeHtml as esc} from './model';
import type {IChartApi,ISeriesApi,UTCTimestamp} from 'lightweight-charts';
const formatter=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'}),day=(t:number)=>formatter.format(t);
export const massiveArchiveCache=new Map<string,{points:OwnGammaObservation[];status:'loaded'|'unavailable';at:number}>();
export function putMassiveArchive(symbol:string,date:string,points:OwnGammaObservation[],at:number,status:'loaded'|'unavailable'='loaded'){
 massiveArchiveCache.set(symbol+':'+date,{points,status,at});if(massiveArchiveCache.size>32)massiveArchiveCache.delete(massiveArchiveCache.keys().next().value!);
}
export interface MassiveChartContext {symbol:string;session:string;cutoff:number;expiry:string;points:OwnGammaObservation[];historical:boolean}
export function massiveChartContext(s:LiveState|undefined,symbol:string,now:number,session?:string,closeMs?:number):MassiveChartContext {
 if(!Number.isSafeInteger(now)||now<=0)return {symbol,session:'',cutoff:0,expiry:'',points:[],historical:false};
 const own=s?.symbol===symbol?s:undefined;
 const minutes=own?.chartHistory?.minutes.filter(b=>Number.isSafeInteger(b.timestampMs)&&b.timestampMs<=now)??[];
 const latest=minutes.reduce((n,b)=>Math.max(n,b.timestampMs),0);
 const selected=session??(latest?day(latest):day(now));
 const end=closeMs??own?.chartHistory?.sessions.find(s=>s.date===selected)?.closeMs;
 const cutoff=end!=null&&Number.isSafeInteger(end)&&end>0?Math.min(now,end):now;
 const sources=[...(massiveArchiveCache.get(symbol+':'+selected)?.points??[]),...ownDominantHistory.all(symbol,cutoff)];
 const unique=new Map<string,OwnGammaObservation>();
 for(const p of sources){if(p.symbol!==symbol||p.at>cutoff||!Number.isSafeInteger(p.at)||p.at<=0||day(p.at)!==selected)continue;const key=JSON.stringify([p.expiry,p.at]),old=unique.get(key);
  if(old&&[old.sourceMs,old.positive,old.negative,old.complete].some((v,i)=>v!==[p.sourceMs,p.positive,p.negative,p.complete][i]))unique.set(key,{...p,positive:null,negative:null,complete:false});else if(!old)unique.set(key,p);
 }
 const all=[...unique.values()].sort((a,b)=>a.at-b.at);
 const h=own?.optionSnapshotStatus?.horizons?.find(h=>['DAY','NEXT_SESSION'].includes(h.mode)&&h.expirations.length===1);
 const current=h&&own?.optionSnapshotStatus.phase==='ready'?ownHeatDominants([h],h.expirations[0],[],cutoff):null;
 const expiry=current?.expiry??all.map(p=>p.expiry).filter(d=>d>=selected).sort()[0]??all.at(-1)?.expiry??'';
 return {symbol,session:selected,cutoff,expiry,points:all.filter(p=>p.expiry===expiry),historical:selected!==day(now)};
}
export function massiveGammaPanel(s:LiveState|undefined,symbol:string,now:number,visible=true):string {
 const c=massiveChartContext(s,symbol,now),last=c.points.at(-1),clock=last?new Date(last.at).toLocaleString('es-ES',{timeZone:'America/New_York'}):'sin captura de esta sesión';
 return `<div class="massive-gamma-controls"><button data-massive-gamma-toggle aria-pressed="${visible}">${visible?'Ocultar':'Mostrar'} gamma Massive</button><details class="tm-method"><summary>GAMMA OI · MASSIVE · CÁLCULO SNIPERS</summary><p>Fuente exclusiva Massive · ${esc(c.expiry||'sin vencimiento verificable')} · ${c.points.length} capturas · ${esc(clock)} ET${c.historical?' · HISTÓRICO':''}.</p><p>Gamma × OI × multiplicador verificado × precio² × 0,01; neto CALL − PUT. Rayitas verdes/rojas en la hora de recepción. Atenuadas y discontinuas: líderes provisionales de cadena incompleta. Sin interpolar capturas ni inferir inventario dealer. El calor de ejecuciones permanece separado.</p></details></div>`;
}
/** Own-source overlay. Never reads Gexbot state, lines or transport. */
export class MassiveGammaLayer {
 private canvas?:HTMLCanvasElement;private signature='';
 private context?:MassiveChartContext;private bars?:number[];private barMs=0;private revision=0;private marks:DominantMark[]=[];
 constructor(private host:HTMLElement,private chart:IChartApi,private candles:ISeriesApi<'Candlestick'>){}
 paint(c:MassiveChartContext,bars:number[],barMs:number,visible=true){
  if(!visible){this.dispose();return;}
  if(this.context!==c||this.bars!==bars||this.barMs!==barMs){this.context=c;this.bars=bars;this.barMs=barMs;this.marks=ownDominantMarks(c.points,c.symbol,c.expiry,c.cutoff,bars,barMs);this.revision++;}
  const width=this.host.clientWidth,height=this.chart.panes()[0].getHeight(),ratio=devicePixelRatio||1,plotWidth=Math.max(0,width-this.chart.priceScale('right').width());
  const signature=JSON.stringify([width,height,ratio,this.chart.timeScale().getVisibleLogicalRange(),this.chart.timeScale().options().barSpacing,this.candles.coordinateToPrice(0),this.candles.coordinateToPrice(height),this.revision]);if(signature===this.signature)return;this.signature=signature;
  this.canvas??=this.host.appendChild(Object.assign(document.createElement('canvas'),{className:'massive-gamma-overlay'}));this.canvas.setAttribute('aria-hidden','true');this.canvas.dataset.gammaSource='massive';
  this.canvas.width=Math.round(width*ratio);this.canvas.height=Math.round(height*ratio);this.canvas.style.width=width+'px';this.canvas.style.height=height+'px';const ctx=this.canvas.getContext('2d');if(!ctx)return;ctx.scale(ratio,ratio);
  const marks=this.marks;
  paintDominantMarks(ctx,marks,plotWidth,height,t=>this.chart.timeScale().timeToCoordinate(t/1000 as UTCTimestamp),this.chart.timeScale().options().barSpacing,p=>this.candles.priceToCoordinate(p),`MASSIVE · GAMMA OI · ${c.expiry||'sin vencimiento'} · ${marks.length} marcas${marks.some(m=>m.provisional)?' · PARCIAL / atenuadas':''}${c.historical?' · HISTÓRICO':''}`);
 }
 dispose(){this.canvas?.remove();this.canvas=undefined;this.signature='';this.context=undefined;this.bars=undefined;this.marks=[];}
}
