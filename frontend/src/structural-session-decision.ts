import type {LiveState} from './live';
import type {SessionStructurePair} from './session-structure-pair';
import type {SessionLocation} from './structural-session-location';
import type {SessionParticipation} from './structural-session-participation';
import type {ChartSignalCoordinatorReport} from './chart-signal-coordinator';
import {ChartSignalDisplay} from './chart-signal-display';
import {escapeHtml as esc} from './model';
export interface SessionDecision {
 version:'structural-session-decision-v1';symbol:string;atMs:number;publishedMs:number|null;
 availability:'current'|'reference'|'pending';state:'PARCIAL'|'TRANSICIÓN'|'PREPARACIÓN'|'CONFIRMACIÓN'|'CONFLICTO';
 side:'CALL'|'PUT'|null;display:string;label:string;reason:string;
 route:{id:string;side:'CALL'|'PUT';reference:number;target:number;invalidation:number;confirmedMs:number;deadlineMs:number;rewardRisk:number}|null;
 probability:null;entryAuthorized:false;
}
/** Passive association with the SAME coordinator that supplies chart arrows. No replay, new gate or order. */
export function readSessionDecision(s:LiveState|undefined,symbol:string,active:boolean,now:number,pair:SessionStructurePair,location:SessionLocation,participation:SessionParticipation,common:Readonly<ChartSignalCoordinatorReport>|null|undefined,origin:'live'|'snapshot'='live'):SessionDecision {
 const r:SessionDecision={version:'structural-session-decision-v1',symbol,atMs:now,publishedMs:s?.publishedMs??null,availability:'pending',state:'PARCIAL',side:null,display:'PARCIAL',label:'Lectura parcial',reason:'Esperando estructuras comparables de5m y15m.',route:null,probability:null,entryAuthorized:false};
 const clock=(v:unknown):v is number=>typeof v==='number'&&Number.isSafeInteger(v)&&v>0&&v<=8640000000000000;
 if(!active||s?.symbol!==symbol||!clock(now)||!clock(s.publishedMs)||s.publishedMs>now||now-s.publishedMs>15000||pair.symbol!==symbol||pair.five.sourceSymbol!==symbol||pair.five.sourcePublishedMs!==s.publishedMs||location.symbol!==symbol||location.observedMs!==now||location.publishedMs!==s.publishedMs||participation.symbol!==symbol||participation.atMs!==now||participation.publishedMs!==s.publishedMs)return r;
 r.availability=origin==='live'&&pair.availability==='current'&&location.availability==='current'&&participation.availability==='current'?'current':'reference';
 if(pair.availability!=='pending'&&pair.side==='MIXTA'){r.state='TRANSICIÓN';r.display='MIXTA';r.label='Estructura en transición';r.reason='5m y15m no sostienen la misma estructura direccional.';}
 else if(pair.availability!=='pending'&&(pair.side==='CALL'||pair.side==='PUT')){
  r.side=pair.side;r.reason=pair.technicalAgreement?'Pivotes y tendencia técnica de5m/15m coinciden. Falta la confirmación propia de la ruta.':'Los pivotes coinciden, pero la tendencia técnica todavía discrepa.';
  if(pair.technicalAgreement){r.state='PREPARACIÓN';r.display=pair.side;r.label='Preparación '+pair.side;}
 }
 const view=new ChartSignalDisplay().read(common,{symbol,startedMs:s.startedMs??null,publishedMs:s.publishedMs,now,active:active&&s.metrics?.phase==='Sesión RTH'&&s.stocks?.phase==='streaming',currentOverlay:r.availability==='current',spot:location.price,origin},true);
 const signal=view.signals.find(v=>v.ticket.identity.strategyId==='session-intraday-v1');
 if(signal){const t=signal.ticket;r.route={id:t.identity.id,side:t.side,reference:t.entryReference!.price,target:t.target!.price,invalidation:t.invalidation!.price,confirmedMs:t.confirmedAtMs,deadlineMs:t.deadlineMs,rewardRisk:t.geometry.rewardRisk};
  if(r.state==='PREPARACIÓN'&&r.side===t.side){r.state='CONFIRMACIÓN';r.display=t.side;r.label='Posible '+t.side+' · ruta confirmada';r.reason='La misma confirmación técnica observada en la gráfica; referencias del subyacente, sin precio de ejecución.';}
  else{r.state='CONFLICTO';r.display='PARCIAL';r.label='Señal y estructura difieren';r.reason='Hay una señal vigente en la gráfica, pero la estructura5m/15m no la confirma. Se conservan ambas lecturas y su origen.';}
 }
 if(r.availability==='reference'){r.label='Referencia · '+r.label;r.reason+=' Referencia de captura o sesión cerrada; no entrada vigente.';}
 return r;
}
export function sessionDecisionDetails(r:SessionDecision):string {
 const money=(n:number)=>'$'+n.toFixed(2),time=(n:number)=>new Date(n).toLocaleString('es',{timeZone:'America/New_York',hour12:false})+' ET';
 return `<details data-horizon-details="structural-session-decision" data-session-decision="${r.state}"><summary>Lectura intradía · preparación, confirmación y recorrido</summary><p>${esc(r.label+'. '+r.reason)}</p>${r.route?`<p>Señal original ${esc(r.route.side)} · ${esc(time(r.route.confirmedMs))}; válida hasta ${esc(time(r.route.deadlineMs))}. Referencia ${money(r.route.reference)}; objetivo ${money(r.route.target)}; invalida por cierre de1m ${money(r.route.invalidation)}. Recorrido/riesgo original ${r.route.rewardRisk.toFixed(4)}. Es la misma oportunidad del gráfico, no otra señal.</p>`:'<p>Sin confirmación vigente asociada: no se inventa una entrada, un objetivo ni una invalidación.</p>'}<p>CALL/PUT describe el subyacente. Los cinco marcos mayores aportan contexto; CVD, volumen y exposición conservan sus unidades. Los apoyos no se suman como votos. No hay probabilidad calibrada ni ejecución de órdenes.</p></details>`;
}
