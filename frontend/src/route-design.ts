import {atrEvidence} from './atr-context';
import {atrPanel,type AtrRead} from './intraday-atr';
import type {ControlReading} from './intraday-control';
import {volatilityReading} from './volatility-reading';
import {rangeFresh} from './daily-range';
import {escapeHtml as esc} from './model';
import './route-design.css';
import {historicalIv} from './iv-history';

export function routeVolatility(r:ControlReading,now=Date.now(),atr?:AtrRead):string{
 const o=r.ivObservation;
 const valid=!!o&&Number.isFinite(o.percent)&&o.percent>0&&rangeFresh({phase:'ready',observedMs:o.at},now);
 const iv=valid?o.percent:null;
 const history=historicalIv(r.symbol,now);
 const rank=history.rank;
 const angle=rank==null?0:-125+rank*2.5;
 const reading=volatilityReading(r,now);
 return `<section class="route-volatility" aria-label="IV Rank 52 semanas"><div class="route-volatility-meters">${atr?atrPanel(atr):""}<div class="route-iv"><h3>IV RANK · 52 SEMANAS</h3><div class="route-gauge" role="img" aria-label="${rank==null?'Reconstruyendo historial':`IV Rank estimado ${rank.toFixed(1)} por ciento`} "><div class="route-gauge-ring"></div>${rank==null?'':`<i class="route-gauge-needle" style="transform:rotate(${angle}deg)"></i>`}<div class="route-gauge-center"><strong>${rank==null?'—':rank.toFixed(1)+'%'}</strong><small>${rank==null?esc(history.progress):'RANK ESTIMADO'}</small></div></div></div></div><p>${esc(rank==null?'RECONSTRUYENDO IV HISTÓRICA':history.label)}</p><details><summary>IV actual e historial</summary>${atrEvidence(atr)}<p>IV de referencia 30 días: ${history.current?.toFixed(2)??'—'}% · ${esc(history.date?history.date+' '+new Date(history.at).toLocaleTimeString('en-US',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit',hour12:false})+' ET':'pendiente')}. Mínima 52 semanas ${history.low?.toFixed(2)??'—'}% · Máxima ${history.high?.toFixed(2)??'—'}%.</p><p>IV ATM de vencimiento próximo: ${iv?.toFixed(2)??'—'}%. ${esc(reading.detail)}</p><small>Rank = (IV de referencia − mínima) / (máxima − mínima) × 100. Serie reconstruida de cotizaciones históricas Data, ATM a 30 días; modelo europeo forward con descuento 1. Aproximación: no reproduce ejercicio americano, dividendos ni la metodología interna de thinkorswim. Fecha indicada; no es probabilidad ni señal de entrada. ${esc(history.reason)}</small></details></section>`;
}
