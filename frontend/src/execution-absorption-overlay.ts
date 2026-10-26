import {absorptionCandidateDetail,type ExecutionAbsorptionCandidate,type ExecutionAbsorptionView} from './execution-absorption-reading';
import {escapeHtml as esc} from './model';
/** Coordinates come from the existing candle/time scales; this SVG never affects autoscale. */
export function absorptionOverlaySvg(view:ExecutionAbsorptionView,point:(c:ExecutionAbsorptionCandidate)=>{x:number;y:number}|null,width:number,height:number):string {
 if(view.status!=='possible'||!Number.isFinite(width)||!Number.isFinite(height)||width<280||height<70)return '';
 const occupied:number[]=[];let html='';
 for(const c of view.candidates){
  const p=point(c);
  if(!p||![p.x,p.y].every(Number.isFinite)||p.x<0||p.x>width-80||p.y<25||p.y>height-25||occupied.some(y=>Math.abs(y-p.y)<26))continue;
  const x=Math.max(4,Math.min(width-244,p.x+12)),color=c.absorbingSide==='buy'?'#00f5ad':'#ff4568';
  const captured=view.capturedAtMs!=null,label=captured?'OBS. EN CAPTURA':`ABS. ${c.absorbingSide==='buy'?'COMPRA':'VENTA'} EST.`;
  html+=`<g><title>${esc((captured?view.detail+' ':'')+absorptionCandidateDetail(c))}</title><line x1="${p.x-6}" x2="${Math.min(width-80,p.x+8)}" y1="${p.y}" y2="${p.y}" stroke="${color}" stroke-width="3"/><rect x="${x}" y="${p.y-11}" width="164" height="22" rx="5" fill="#08182c" fill-opacity=".94" stroke="${color}"/><text x="${x+7}" y="${p.y+4}" fill="${color}" font-size="10" font-weight="700">${label} · $${c.price.toFixed(2)}</text></g>`;
  occupied.push(p.y);if(occupied.length===2)break;
 }
 return html;
}
