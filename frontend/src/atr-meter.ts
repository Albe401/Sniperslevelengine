import type {AtrRead} from './intraday-atr';
import {escapeHtml as esc} from './model';
/** Inline magnitude gauge; never infers exhaustion or a direction. */
export function atrMeter(a:AtrRead|undefined,ratio=a?.context?.rthRangeDaily,label='Rango RTH / ATR diario'):string {
 const valid=ratio!=null&&Number.isFinite(ratio)&&ratio>=0;
 const amount=valid?ratio!*100:null,width=amount==null?0:Math.min(100,amount/1.5);
 const color=amount==null?'#52657d':amount>=150?'#ff5277':amount>=100?'#ffdc36':'#35dca4';
 const text=amount==null?'pendiente':amount.toFixed(0)+'%';
 const title=label+' · '+text+' · escala 0–150%; el recorrido puede superar la escala. Extensión de rango; no confirma agotamiento ni giro.';
 return `<span class="atr-meter" role="img" aria-label="${esc(title)}" title="${esc(title)}"><span class="atr-meter-track"><i style="width:${width.toFixed(2)}%;background:${color}"></i></span><span class="atr-meter-value">${text}</span></span>`;
}
