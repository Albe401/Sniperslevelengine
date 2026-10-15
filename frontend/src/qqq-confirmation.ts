import type {LiveState} from './live';
import {currentComponentRead,usableComponentReport,type ComponentRead} from './qqq-components';
export interface QqqConfirmation {status:'confirms'|'contradicts'|'mixed'|'pending';label:string;detail:string;at:number;call:number|null;put:number|null}
/** Current basket pressure confirms context; never changes probabilities or structural exposure. */
export function qqqConfirmation(s:LiveState|undefined,symbol:string,active:boolean,side:string,now=Date.now()):QqqConfirmation|undefined {
 if(symbol!=='QQQ')return undefined;
 const pending:QqqConfirmation={status:'pending',label:'Componentes · pendientes',detail:'Esperando evidencia vigente del QQQ y cobertura suficiente de sus componentes.',at:now,call:null,put:null};
 const r:ComponentRead|undefined=s?.qqq_report===undefined?currentComponentRead(now):s.qqq_report??undefined;
 if(!active||s?.symbol!==symbol||s.stocks?.phase!=='streaming'||!Number.isFinite(s.publishedMs)||s.publishedMs>now||now-s.publishedMs>3000||!usableComponentReport(r,now)||!r.available||r.call==null||r.put==null)return pending;
 const basket=r.call>=60?'CALL':r.call<=40?'PUT':'NEUTRAL';
 const directional=side==='CALL'||side==='PUT';
 const status=!directional?'pending':basket==='NEUTRAL'?'mixed':basket===side?'confirms':'contradicts';
 const label=status==='confirms'?'Componentes · confirman':status==='contradicts'?'Componentes · divergencia':status==='mixed'?'Componentes · mixtos':'Componentes · sin escenario';
 return {status,label,at:r.at,call:r.call,put:r.put,detail:label+' · '+r.phase+' · CALL '+r.call+'% / PUT '+r.put+'%. '+r.reason+' Presión actual de la cesta; no confirma por sí sola ruptura, absorción ni dirección semanal. No se suma a porcentajes ni modifica niveles.'};
}
