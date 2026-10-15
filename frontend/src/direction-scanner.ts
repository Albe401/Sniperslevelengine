import {qqqConfirmation} from './qqq-confirmation';
import {premarketDirection} from './premarket-direction';
import {combineDirection} from './direction-model';
import {visibleRouteReading,type ProbabilityReading} from './route-probability-reading';
import {readMarketEvidence,marketCvdReading} from './market-evidence';
import {vannaConfluence,type VannaRead} from './vanna-confluence';
import {atrMeter} from './atr-meter';
import {atrEvidence} from './atr-context';
import type {AtrRead} from './intraday-atr';
import {preparation} from './direction-preparation';
import {cvdConfluence,type CvdConfluence} from './cvd-confluence';
import {treasuryContext} from './treasury';
import type {LiveState} from './live';
import type {ControlReading} from './intraday-control';
import {mapRouteLevels} from './route-level-context';
import {interpretationFresh} from './confirmations';
import {escapeHtml as esc} from './model';
export interface DirectionScore {vanna?:VannaRead;cvd?:CvdConfluence;macro?:{label:string;detail:string};context?:string;call:number|null;put:number|null;side:'CALL'|'PUT'|'NEUTRAL'|'PENDIENTE';state:string;reason:string;components:{name:string;signal:number;weight:number}[]}
const clamp=(n:number)=>Math.max(-1,Math.min(1,n));
const finite=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n);
const pending=(reason:string):DirectionScore=>({call:null,put:null,side:'PENDIENTE',state:'Esperando datos',reason,components:[]});
/** Experimental confluence, not a calibrated success probability or entry trigger. */
export {combineDirection} from './direction-model';
function fullDirectionScore(s:LiveState|undefined,r:ControlReading,active:boolean,now:number):DirectionScore {
 const evidence=readMarketEvidence(s,r.symbol,active,now,r.marketEvidence?undefined:{price:r.priceNow,signal:r.components?.find(c=>c.name==='PRECIO 2M + VWAP')?.signal});
 return {...evidence.score,cvd:r.marketEvidence?marketCvdReading(evidence):undefined,reason:evidence.score.reason+' '+evidence.detail};
}
export function readDirectionScore(s:LiveState|undefined,r:ControlReading,active:boolean,now=Date.now()):DirectionScore {
 const full=fullDirectionScore(s,r,active,now);
 const initial=preparation(s,r,active,now);
 const result=full.call!=null?full:premarketDirection(s,r.symbol,active,now)??initial??full;
 if(full.call!=null&&initial?.context)result.reason+=' '+initial.context;
 result.cvd=full.cvd??cvdConfluence(s,result,r.symbol,active,now);
 result.vanna=vannaConfluence(s,result,r.symbol,active,now);
 result.macro=treasuryContext(s?.treasury,now);const basket=qqqConfirmation(s,r.symbol,active,result.side,now);if(basket)result.reason+=' '+basket.detail;return result;
}
export function directionBadge(d:DirectionScore,atr?:AtrRead,probability?:ProbabilityReading,component?:{label:string;detail:string;actors?:string;labelMarkup?:string}):string {
 if(probability)probability=visibleRouteReading(probability,d);
 const provisional=probability?.provisional;
 const shown=provisional??probability??d;
 const side=probability?.kind==='baseline'?'NEUTRAL':provisional?d.side:probability?(probability.call!=null?'CALL':probability.put!=null?'PUT':'PENDIENTE'):d.side;
 const tone=d.state==='Lectura mixta'?'neutral':side==='CALL'?'call':side==='PUT'?'put':'neutral';
 return `<aside class="direction-badge direction-${tone}${component?.actors?' qqq-split-badge':''}" aria-label="Escáner de dirección CALL PUT">${component?.actors?'<section class="qqq-direction-main">':''}<span>${probability?.kind==='baseline'?'LLEGADA':'CALL / PUT'} · ${probability?.kind==='baseline'?'P. HOY · EXP.':provisional?'CONF. PROVISIONAL':probability?'PROB. 10 MIN':'CONFLUENCIA'}</span><div><b>${probability?.kind==='baseline'?'SUBIR':'CALL'} <strong>${shown.call==null?'—':shown.call+'%'}</strong></b><b>${probability?.kind==='baseline'?'BAJAR':'PUT'} <strong>${shown.put==null?'—':shown.put+'%'}</strong></b></div><small>${esc(provisional?'Sin calibrar · '+d.state:probability?.state??d.side+' · '+d.state)}</small>${d.cvd?`<small class="direction-cvd direction-cvd-${d.cvd.status}">${esc(d.cvd.label)}</small>`:''}${!component?.actors?component?`<small class="direction-treasury qqq-component-line" data-qqq-component-line title="${esc(component.detail)}">${esc(component.label)}</small>`:d.macro?`<small class="direction-treasury">${esc(d.macro.label)}</small>`:'':''}<details data-horizon-details="direction-score"><summary class="atr-inline-summary" title="${esc(atr?.context?.detail??'')}">${esc(atr?.context?.label??'Ver datos')} ${atrMeter(atr)}</summary>${atrEvidence(atr)}${probability?`<p>${esc(probability.detail)}</p><p>Confluencia interna: CALL ${d.call??'—'} / PUT ${d.put??'—'}; índice descriptivo, no probabilidad. Se conserva como contexto.</p>`:''}<p>${esc(d.reason)}</p>${d.cvd?`<p>${esc(d.cvd.detail)}</p>`:''}${d.macro?`<p>${esc(d.macro.detail)}</p>`:''}${d.components.map(c=>`<p>${esc(c.name)}: ${c.signal.toFixed(2)} · peso ${c.weight}</p>`).join('')}<p>${probability?'Los dos lados no son complementarios: cada uno necesita su escenario y validación.':'Porcentajes de confluencia; no probabilidad de éxito.'} Sin órdenes ni estimación de hora.</p></details>${component?.actors?`</section><section class="qqq-direction-components"><span class="qqq-components-heading">Componentes QQQ</span><small class="direction-treasury qqq-component-line" data-qqq-component-line title="${esc(component.detail)}">${component.labelMarkup??esc(component.label)}</small><div data-qqq-component-actors>${component.actors}</div></section>`:''}</aside>`;
}
