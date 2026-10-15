import type {AuctionReading} from './auction-map-panel';
import {rejectionEvidence} from './auction-map-panel';
import {auctionDisplayDay} from './auction-display-memory';
import {profileGroups} from './auction-volume-profile';
import {escapeHtml as esc} from './model';
export interface ChartAuctionProfile {reading:AuctionReading;state:'live'|'retained'}
/** Same-day observed tick profile only. Never invent a profile from candle volume. */
export function chartAuctionProfile(symbol:string,active:boolean,now:number,current?:AuctionReading,retained?:AuctionReading):ChartAuctionProfile|undefined {
 const valid=(r:AuctionReading|undefined):r is AuctionReading=>!!r&&r.symbol===symbol&&r.version==='auction-map-ticks-v1'&&Number.isFinite(r.asOfMs)&&r.asOfMs<=now&&auctionDisplayDay(r.asOfMs)===auctionDisplayDay(now)&&r.acceptedTrades>0&&Number.isFinite(r.totalVolume)&&r.totalVolume>0&&r.bins.length>0&&r.bins.length<=20001&&r.bins.every(b=>[b.low,b.high,b.volume,b.volumeRatio,b.timeRatio].every(Number.isFinite)&&b.low>0&&b.high>b.low&&b.volume>=0);
 if(valid(current)&&active&&now-current.asOfMs<=15000)return {reading:current,state:'live'};
 const candidates=[current,retained].filter(valid).sort((a,b)=>b.asOfMs-a.asOfMs);
 return candidates[0]?{reading:candidates[0],state:'retained'}:undefined;
}
/** Geometry uses the candle scale; SVG is pointer-transparent and never changes autoscale. */
export function auctionChartBarsSvg(view:ChartAuctionProfile|undefined,coordinate:(price:number)=>number|null,width:number,height:number,now:number):string {
 if(!view||![width,height].every(Number.isFinite)||width<160||height<40)return '';
 const r=view.reading,groups=profileGroups(r.bins,Math.min(180,Math.max(1,Math.floor(height/2))));
 const maximum=Math.max(0,...groups.map(b=>b.volume));if(maximum<=0||!Number.isFinite(maximum))return '';
 const maxWidth=Math.min(140,width*.18),upper=view.state==='live'?rejectionEvidence(r.rejectionUpper,r.symbol,now):undefined,lower=view.state==='live'?rejectionEvidence(r.rejectionLower,r.symbol,now):undefined;
 let html='';
 for(const b of groups){
  const a=coordinate(b.high),z=coordinate(b.low);if(a==null||z==null||![a,z].every(Number.isFinite))continue;
  const top=Math.max(0,Math.min(a,z)),bottom=Math.min(height,Math.max(a,z));if(bottom<=top||b.volume<=0)continue;
  const poc=b.low<r.pocHigh&&b.high>r.pocLow,rejected=[upper,lower].some(v=>v&&v.level>=b.low&&v.level<b.high);
  const tone=poc?'poc':rejected?'rejection':b.acceptance?'acceptance':'volume',color=poc?'#ac8be7':rejected?'#ff6273':b.acceptance?'#20bb89':'#276b89';
  const barWidth=maxWidth*b.volume/maximum,opacity=view.state==='retained'?.22:poc?.62:b.acceptance||rejected?.44:.30;
  const label=`${r.symbol} · ${b.low.toFixed(2)}–${b.high.toFixed(2)} · ${b.volume.toFixed(6)} acciones · ${tone} · ${view.state==='retained'?'perfil guardado, sin vigencia':'perfil observado vigente'} · referencia ${r.asOfMs}`;
  html+=`<rect data-amt-bar="${tone}" data-low="${b.low}" data-high="${b.high}" x="0" y="${top.toFixed(3)}" width="${barWidth.toFixed(3)}" height="${Math.max(.5,bottom-top-.35).toFixed(3)}" fill="${color}" fill-opacity="${opacity}" stroke="${color}" stroke-opacity="${view.state==='retained'?.4:.8}" stroke-width=".45"><title>${esc(label)}</title></rect>`;
 }
 return html;
}
