import type {AtrRead} from './intraday-atr';
import {escapeHtml as esc} from './model';
import {optionsCvdPanel} from './options-cvd';
import type {LiveState} from './live';
import {interpretationFresh} from './confirmations';
/** Share of classified BUY premiums only. Neither market coverage nor price probability. */
export function buyPremiumPercentages(call:unknown,put:unknown,complete:boolean):{call:number;put:number}|null {
 if(!complete||typeof call!=='number'||typeof put!=='number'||!Number.isFinite(call)||!Number.isFinite(put)||call<0||put<0)return null;
 const total=call+put;if(!Number.isFinite(total)||total<=0)return null;
 const percent=100*(call/total);return {call:percent,put:100-percent};
}
export function flowFour(state:LiveState|undefined,symbol:string,active:boolean,now=Date.now(),atr?:AtrRead,includeConfluence=true):string {
 const usable=active&&state?.symbol===symbol&&interpretationFresh(state.publishedMs,now)&&state.options.phase==='streaming';
 const sameDay=usable?state.flow.intraday_windows?.find(w=>w.minutes===5):undefined;
 const window=sameDay??(usable?state.flow.windows?.find(w=>w.minutes===5):undefined);
 const scope=sameDay?'Vencimiento de hoy':'Vencimientos seguidos';
 const shares=window&&window.event_count>0?buyPremiumPercentages(window.buckets?.['CALL BUY']?.premium,window.buckets?.['PUT BUY']?.premium,window.premium_complete===true):null;
 const ratioDetail='Porcentaje de prima compradora clasificada en 5m: CALL BUY / (CALL BUY + PUT BUY). Ventas y prima desconocida excluidas; no mide cobertura ni probabilidad.';
 const at=state?.options.lastEventMs;
 const age=at!=null&&at<=now+1000?Math.max(0,Math.floor((now-at)/1000)):null;
 const status=!usable?'Sin conexión vigente':!window?'Esperando ventanas de flujo':!window.event_count?'Sin operaciones observadas en 5m':age==null?'Hora del último evento no verificable':`Último evento de opciones: hace ${age} s`;
 const money=(n:number)=>Intl.NumberFormat('en-US',{style:'currency',currency:'USD',maximumFractionDigits:0}).format(n);
 return `<section class="flow-four flow-four-simple" aria-label="Primas compradoras de opciones observadas en los últimos cinco minutos">${['CALL BUY','PUT BUY'].map((label,i)=>{
 const value=window?.buckets?.[label]?.premium;
 const observed=!!window&&window.event_count>0;
 const valid=observed&&value!=null&&Number.isFinite(value)&&value>=0;
 const unknown=window?.buckets?.UNKNOWN?.premium;
 const unknownText=unknown!=null&&Number.isFinite(unknown)&&unknown>0?` · sin clasificar ${money(unknown)}`:'';
 const detail=!usable?'Sin flujo vigente':!window?'Esperando datos':!observed?'Sin operaciones recibidas':!valid?'Prima no verificable':value===0?'Sin compras clasificadas':`Prima hoy${window.premium_complete?'':' · parcial'}`;
 const ratio=shares?` · ${(i===0?shares.call:shares.put).toFixed(2)}%`:'';
 return `<div class="flow-four-cell ${i===0||i===3?'flow-green':'flow-red'}" title="${esc(status+unknownText)} · ${scope} · lado inferido · ${esc(ratioDetail)}${shares?'':' · porcentaje pendiente'}"><span>${label}${ratio}</span><strong>${valid?money(value!):'—'}</strong><small>${esc(detail)}</small></div>`;
 }).join('')}${includeConfluence?optionsCvdPanel(state,symbol,active,now,atr):""}</section>`;
}
