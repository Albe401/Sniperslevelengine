import type {LiveState} from './live';
import {escapeHtml as esc} from './model';
export interface SwingReport {mode:string;state:string;context:{version:string;symbol:string;at:number;side:string;blockers:string[]};plan?:{side:string;anchor:{price:number};target:{price:number}}|null;intraday:{state:string};remainingDollars?:number|null;remainingDailyAtr?:number|null;reason:string;journalError?:string|null}
export function swingObservation(s:LiveState|undefined,symbol:string,active:boolean,now:number){
 const r=s?.swingReport;
 if(!active||s?.symbol!==symbol||!r||r.mode!=='observation-only'||r.context?.version!=='structural-swing-v1'||r.context.symbol!==symbol||!Number.isFinite(r.context.at)||r.context.at>now||now-r.context.at>5000)return {label:'SWING · PENDIENTE',html:'<small>Contexto swing pendiente de datos vigentes.</small>'};
 const labels:Record<string,string>={observing:'en observación',divergence:'divergencia',suspended:'suspendido',invalidated:'invalidado',reached:'referencia alcanzada',expired:'vencido'};
 const amount=(n:unknown)=>typeof n==='number'&&Number.isFinite(n)&&n>=0?`$${n.toFixed(2)}`:'—';
 const lines=[`SWING · ${r.context.side} · ${labels[r.state]??r.state}`,r.reason,`Confirmación intradía: ${r.intraday.state}`];
 if(r.plan)lines.push(`Ancla ${amount(r.plan.anchor.price)} · referencia ${amount(r.plan.target.price)} · distancia ${amount(r.remainingDollars)}`);
 if(typeof r.remainingDailyAtr==='number'&&Number.isFinite(r.remainingDailyAtr))lines.push(`Distancia: ${r.remainingDailyAtr.toFixed(2)} ATR diario`);
 lines.push(...r.context.blockers);if(r.journalError)lines.push(r.journalError);
 lines.push('Observación registrada; sin ejecución automática.');
 return {label:lines[0],html:lines.map(t=>`<small>${esc(t)}</small>`).join('')};
}
