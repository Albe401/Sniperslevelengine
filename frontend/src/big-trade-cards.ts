import {absorptionForTrade,absorptionSideLabel,absorptionCandidateDetail,type ExecutionAbsorptionCandidate} from './execution-absorption-reading';
import type {BigTrade} from './cvd';
import {escapeHtml as esc} from './model';
const quantity=new Intl.NumberFormat('en-US',{maximumFractionDigits:0});
const timeET=new Intl.DateTimeFormat('es',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit',second:'2-digit',hour12:false});
export function recentBigTrades(trades:BigTrade[]):BigTrade[]{return [...trades].sort((a,b)=>b.at-a.at||b.sequence-a.sequence).slice(0,5);}
/** Five freshest observed prints; new arrivals replace the oldest automatically. */
export class BigTradeTape {
 observe(trades:BigTrade[],_key:string,now=Date.now()){return recentBigTrades(trades.filter(t=>Number.isFinite(t.at)&&t.at<=now+1000&&now-t.at<60000));}
}
export function bigTradeToolbar(){return `<button data-execution-absorption aria-pressed="true" title="Posible absorción estimada a partir de operaciones y NBBO; no iceberg confirmado">Absorción · pendiente</button><div class="gl-big-trade-bar" id="gl-big-trade-bar" data-chart-preserve><div class="gl-big-trade-controls"><button data-big-trades title="Operaciones individuales observadas; lado estimado por bid/ask."></button><select data-big-trades-min aria-label="Importe mínimo del trade grande"><option value="100000">Trades ≥ $100 mil</option><option value="250000">Trades ≥ $250 mil</option><option value="1000000">Trades ≥ $1 millón</option></select></div><div class="gl-big-trade-tape" aria-label="Trades grandes · cantidad, precio, importe y hora ET" tabindex="0"></div></div>`;}
export function bigTradeCards(trades:BigTrade[],selected:number|undefined,candidates:ExecutionAbsorptionCandidate[]=[]):string{
 return recentBigTrades(trades).map(t=>{
  const side=t.side===1?'Compra est.':t.side===-1?'Venta est.':'Sin clasificar',color=t.side===1?'#00f5ad':t.side===-1?'#ff4568':'#ffd75a';
  const time=timeET.format(t.at),shares=quantity.format(t.shares);
  const absorption=absorptionForTrade(t,candidates);
  const detail=`${side} · ${shares} acciones @ $${t.price.toFixed(2)} · $${quantity.format(t.notional)} · ${time} ET · lado inferido; no confirma apertura ni intención`;
  return `<button class="gl-trade-card" data-trade-sequence="${t.sequence}" style="--trade-color:${color}" aria-pressed="${selected===t.sequence}" title="${esc(detail+(absorption?' · Zona actual: '+absorptionCandidateDetail(absorption):''))}"><span>${side}${absorption?' · '+absorptionSideLabel(absorption):''}</span><div><b>${shares}<small> acc.</small></b><time>${time}</time></div><small class="gl-trade-price">$${t.price.toFixed(2)} · $${quantity.format(t.notional)}</small></button>`;
 }).join('');
}
