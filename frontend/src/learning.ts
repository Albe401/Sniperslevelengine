import {learningHealthPanel} from './learning-health';
import {structureStudyPanel,refreshStructureStudy,wireStructureStudy} from './structure-study';
import {botLearningPanel,type BotLearning} from './bot-learning';
import {systemAnalysis,systemAnalysisPanel} from './system-analysis';
import {routeChartReading} from './route-live';
import {momentumShadow,momentumPanel} from './momentum-shadow';
import {continuousAudit,continuousAuditPanel} from './continuous-quick-audit';
import {invoke,isTauri} from './snapshot-observation';
import {escapeHtml as esc} from './model';
import {updateLiveWorkspace} from './live-dom';
import './learning.css';

interface Outcome {minutes:number;status:string;returnBps:number|null;outcomeMs:number|null}
export interface LearningReport {
 directionalCohorts?:{symbol:string;build:string;side:string;minutes:number;observed:number;sessions:number;aligned:number;flat:number;meanDirectionalBps:number|null}[];
 cohortScope?:string;
 qqqComponents?:{readings:number;scope:string;outcomes:{minutes:number;complete:number;directional:number;aligned:number;meanReturnBps:number|null}[]};
 bot?:BotLearning;
 gammaMap?:{observations:number;eligible:number;recent:LearningReport['recent'];outcomes:LearningReport['outcomes']};
 gammaCollector?:{recorded:boolean;observed_ms:number;symbol:string;quality:string}|null;
 exists:boolean;connected:boolean;generatedMs:number;symbols:string[];observations:number;eligible:number;sessions:number;buildSessions:number;
 collector:{observed_ms:number;recorded:boolean;eligible:boolean;symbol:string;quality:string;error:string|null}|null;
 bridge:{capturedMs:number;symbol:string;quality:string;hasContext:boolean;stocksPhase:string;optionsPhase:string}|null;
 quality:{reason:string;count:number}[];
 outcomes:{minutes:number;observed:number;pending:number;missing:number;meanBps:number|null}[];
 recent:{id:number;symbol:string;session:string;observedMs:number;price:number|null;eligible:boolean;quality:string;side:string|null;build:string|null;outcomes:Outcome[]}[];
}
let data:LearningReport|undefined,busy=false,error='',filter='';
let root:Element|null=null,timer:ReturnType<typeof setInterval>|undefined,generation=0;
const count=(v:number|undefined)=>v==null?'—':v.toLocaleString('es-US');
const time=(ms:number|undefined)=>ms==null?'Sin registro':new Date(ms).toLocaleString('es-US',{timeZone:'America/New_York',month:'short',day:'2-digit',hour:'2-digit',minute:'2-digit',second:'2-digit',hour12:false})+' ET';
const fresh=(ms:number|undefined,now:number,age:number)=>ms!=null&&ms<=now&&now-ms<=age;
const quality=(v:string)=>({READY:'Lectura válida',NO_FRESH_PRICE:'Sin precio vigente',OUTSIDE_REGULAR_SESSION:'Fuera de sesión regular',INCOMPLETE_INTRADAY_STRUCTURE:'Estructura incompleta',CONTEXT_UNAVAILABLE:'Contexto incompleto',SYNTHETIC_FIXTURE:'Prueba sintética'}[v]??'Datos por verificar');
const percent=(bps:number|null)=>bps==null||!Number.isFinite(bps)?'—':`${bps>0?'+':''}${(bps/100).toFixed(3)}%`;

export function recordingState(d:LearningReport|undefined,now:number):{label:string;tone:string;detail:string} {
 if(!d)return {label:'Sin consulta',tone:'muted',detail:'Abre esta sección en la aplicación de Windows.'};
 if(d.collector?.error&&fresh(d.collector.observed_ms,now,90_000))return {label:'Revisar registro',tone:'bad',detail:'No se pudo guardar la última lectura. El historial anterior se conserva.'};
 if(d.connected&&d.collector?.recorded&&fresh(d.collector.observed_ms,now,75_000))return {label:'Registro activo',tone:'good',detail:`${d.collector.symbol} · ${quality(d.collector.quality)}`};
 return {label:d.exists?'Registro en pausa':'Esperando conexión',tone:'muted',detail:'Conecta Mercado en vivo para registrar nuevas lecturas.'};
}
function outcomeCell(row:LearningReport['recent'][number],minutes:number):string {
 const o=row.outcomes.find(v=>v.minutes===minutes);
 if(!o)return '<span class="learn-muted">—</span>';
 if(o.status!=='observed')return `<span class="learn-muted">${o.status==='pending'?'En seguimiento':'Sin observación'}</span>`;
 return `<span class="${o.returnBps!=null&&o.returnBps>0?'learn-positive':o.returnBps!=null&&o.returnBps<0?'learn-negative':''}" title="${esc(time(o.outcomeMs??undefined))}">${percent(o.returnBps)}</span>`;
}
export function learningContent(d:LearningReport|undefined,loading=false,message='',symbol='',now=Date.now()):string {
 const analysisSymbol=symbol||d?.bridge?.symbol||null;
 const analysis=systemAnalysis(d,continuousAudit.records,analysisSymbol,now,analysisSymbol?routeChartReading(analysisSymbol,now)?.directional:undefined);
 const state=recordingState(d,now),observed=d?.outcomes.reduce((n,o)=>n+o.observed,0);
 const bridgeLive=!!d?.connected&&!!d.bridge?.hasContext&&fresh(d.bridge.capturedMs,now,10_000);
 const days=d?.buildSessions??0;
 return `<section id="learning-workspace" class="learning-workspace" aria-label="Aprendizaje y análisis">
 <header class="learn-hero"><div><span class="learn-eyebrow">INTELIGENCIA · MEMORIA DEL MOTOR</span><h1>Aprendizaje <span>y análisis</span></h1><p>Cada lectura deja evidencia. Cada resultado permite evaluar la siguiente mejora.</p></div><div class="learn-hero-actions"><span class="learn-badge ${message?'bad':state.tone}" role="status">${message?'Consulta no disponible':state.label}</span><button class="learn-button" data-learning-refresh ${loading?'disabled':''}>${loading?'Actualizando…':'Actualizar'}</button></div></header>
 ${message?`<div class="learn-error" role="alert">${esc(message)}${d?' Los valores visibles corresponden a la última consulta.':''}</div>`:''}
 <div class="learn-toolbar"><label for="learning-symbol">Historial de</label><select id="learning-symbol" data-learning-filter><option value="">Todos los activos</option>${[...new Set([...(d?.symbols??[]),...(d?.bot?.symbols??[]),...(symbol?[symbol]:[])])].map(s=>`<option value="${esc(s)}" ${s===symbol?'selected':''}>${esc(s)}</option>`).join('')}</select><span>Consulta: ${esc(time(d?.generatedMs))}</span></div>
 ${structureStudyPanel()}${learningHealthPanel(d,now)}${systemAnalysisPanel(analysis)}${momentumPanel()}${continuousAuditPanel()}${botLearningPanel(d?.bot)}${d?.qqqComponents?`<section class="learn-panel"><h2>Componentes QQQ · evaluación v2</h2><p>${d.qqqComponents.readings} lecturas · ${esc(d.qqqComponents.scope)}</p><p>${d.qqqComponents.outcomes.map(o=>`${o.minutes} min: ${o.complete} resultados válidos, ${o.aligned}/${o.directional} direcciones acompañadas`).join(' · ')}</p><small>Recorrido del QQQ, sin PnL de opciones ni ajuste automático del bot. Faltantes excluidos; premarket y RTH se registran por separado en el diario.</small></section>`:''}
 <div class="learn-stats">${[['Lecturas guardadas',count(d?.observations),'Contextos persistidos'],['Aptas para evaluación',count(d?.eligible),'Precio y estructura vigentes'],['Resultados observados',count(observed),'Total de ventanas 5 · 15 · 30 min'],['Sesiones registradas',count(d?.sessions),'Del historial seleccionado']].map(([label,value,detail],i)=>`<article class="learn-stat color-${i}"><span>${label}</span><strong>${value}</strong><small>${detail}</small></article>`).join('')}</div>
 <div class="learn-main-grid"><section class="learn-panel"><div class="learn-section-head"><h2>Conexión con el motor</h2><span class="learn-badge ${bridgeLive?'good':'muted'}">${bridgeLive?'Contexto vigente':'Sin contexto vigente'}</span></div><div class="learn-link"><div class="learn-orbit ${bridgeLive?'active':''}" aria-hidden="true">S</div><div><strong>${esc(d?.collector?.symbol??'Motor en espera')}</strong><p>${esc(state.detail)}</p></div></div><dl class="learn-facts"><div><dt>Última lectura guardada</dt><dd>${esc(time(d?.collector?.recorded?d.collector.observed_ms:undefined))}</dd></div><div><dt>Contexto para análisis</dt><dd>${esc(time(d?.bridge?.capturedMs))}</dd></div><div><dt>Frecuencia del registro</dt><dd>30 segundos · sin reiniciar el motor</dd></div></dl><button class="learn-link-button" data-learning-market>Ir a Mercado en vivo →</button></section>
 <section class="learn-panel learn-evolution"><div class="learn-section-head"><h2>Evolución de la lectura</h2><span class="learn-badge cyan">OBSERVACIÓN</span></div><ol class="learn-stages"><li class="current"><b>01</b><div><strong>Recopilar evidencia</strong><p>Lecturas, cobertura y resultados reales.</p></div></li><li><b>02</b><div><strong>Comparar un candidato</strong><p>${days<10?`${days} de 10 sesiones mínimas con la misma versión.`:'Mínimo de sesiones alcanzado; falta verificar muestras y resultados.'}</p></div></li><li><b>03</b><div><strong>Validar la mejora</strong><p>Evaluación posterior con datos nuevos.</p></div></li></ol><p class="learn-note">El evaluador experimental se ejecuta por separado. Todavía no hay una mejora validada ni cambios automáticos de reglas.</p></section></div>
 <section class="learn-panel"><div class="learn-section-head"><div><h2>Qué pasó después</h2><p>Movimiento del subyacente desde cada lectura. No representa ganancias de opciones ni tasa de acierto.</p></div></div><div class="learn-horizons">${[5,15,30].map(m=>{const o=d?.outcomes.find(v=>v.minutes===m);return `<article><div class="learn-horizon-label"><strong>${m}<small> min</small></strong><span>Ventana de observación</span></div><dl><div><dt>Observados</dt><dd>${count(o?.observed??(d?0:undefined))}</dd></div><div><dt>En seguimiento</dt><dd>${count(o?.pending??(d?0:undefined))}</dd></div><div><dt>Sin observación</dt><dd>${count(o?.missing??(d?0:undefined))}</dd></div><div class="learn-return"><dt>Movimiento medio</dt><dd>${percent(o?.meanBps??null)}</dd></div></dl></article>`;}).join('')}</div></section>
 <section class="learn-panel"><div class="learn-section-head"><div><h2>Lecturas recientes</h2><p>Últimas 25 · hora de Nueva York · las ausencias se conservan como datos faltantes.</p></div></div><div class="learn-table-scroll"><table><thead><tr><th>Activo / hora</th><th>Precio</th><th>Lectura</th><th>Calidad</th><th>5 min</th><th>15 min</th><th>30 min</th></tr></thead><tbody>${d?.recent.map(r=>`<tr><td><strong>${esc(r.symbol)}</strong><small>${esc(time(r.observedMs))}</small></td><td>${r.price==null?'—':`$${r.price.toFixed(2)}`}</td><td>${esc(r.side??'—')}</td><td><span class="learn-quality ${r.eligible?'good':''}">${quality(r.quality)}</span></td>${[5,15,30].map(m=>`<td>${outcomeCell(r,m)}</td>`).join('')}</tr>`).join('')||`<tr><td colspan="7" class="learn-empty"><strong>${loading?'Consultando el historial…':message?'No se pudo consultar el historial':d?'Todavía no hay lecturas guardadas':'Historial disponible en Windows'}</strong><p>${d?'Las primeras lecturas aparecerán al conectar Mercado en vivo.':'Este panel muestra datos reales guardados por la aplicación.'}</p></td></tr>`}</tbody></table></div></section>
 <section class="learn-panel"><div class="learn-section-head"><div><h2>Mapa gamma · memoria de lecturas</h2><p>Dominantes, fuerza y cambios CALL ↔ PUT · versión gamma-map-v1 · registro cada 30 s y al cambiar el estado.</p></div><span class="learn-badge ${d?.gammaCollector?.recorded&&fresh(d.gammaCollector.observed_ms,now,75000)?'good':'muted'}">${d?.gammaCollector?.recorded&&fresh(d.gammaCollector.observed_ms,now,75000)?'Registro gamma activo':'Esperando registro gamma'}</span></div><p>${count(d?.gammaMap?.observations)} lecturas · ${count(d?.gammaMap?.eligible)} aptas para evaluación</p><div class="learn-table-scroll"><table><thead><tr><th>Activo / hora</th><th>Lectura gamma</th><th>5 min</th><th>15 min</th><th>30 min</th></tr></thead><tbody>${d?.gammaMap?.recent.map(r=>`<tr><td>${esc(r.symbol)}<small>${esc(time(r.observedMs))}</small></td><td>${esc(r.side??'Lectura pendiente')}</td>${[5,15,30].map(m=>`<td>${outcomeCell(r,m)}</td>`).join('')}</tr>`).join('')||'<tr><td colspan="5">Conecta Mercado en vivo para registrar el mapa gamma. El demo simulado no alimenta este historial.</td></tr>'}</tbody></table></div><p class="learn-note">Resultados del subyacente; no son ganancias de opciones. Recopilación y evaluación, sin cambios automáticos de reglas.</p></section>
 ${d?.quality.length?`<section class="learn-quality-summary" aria-label="Calidad del historial">${d.quality.map(q=>`<span>${quality(q.reason)} <b>${count(q.count)}</b></span>`).join('')}</section>`:''}
 </section>`;
}
export const learningPage=()=>learningContent(data,busy,error,filter);
function paint(){if(root?.isConnected){updateLiveWorkspace(root,learningContent(data,busy,error,filter));wire();}}
async function load(){
 if(busy||!root?.isConnected)return;
 const token=generation;await refreshStructureStudy();busy=true;error='';paint();
 try {if(!isTauri())throw new Error('Abre la aplicación de Windows para consultar el registro real.');
  const result=await invoke<LearningReport>('get_learning_dashboard',{symbol:filter||null});
  if(token===generation)data=result;
 }catch(e){if(token===generation)error=typeof e==='string'?e:e instanceof Error?e.message:'No se pudo consultar el registro de aprendizaje.';}
 finally{if(token===generation){busy=false;paint();}}
}
function wire(){
 wireStructureStudy(root,paint);
 const momentum=root?.querySelector<HTMLButtonElement>('[data-momentum-toggle]');if(momentum)momentum.onclick=()=>{momentumShadow.toggle();paint();};
 const toggle=root?.querySelector<HTMLButtonElement>('[data-quick-audit-toggle]');if(toggle)toggle.onclick=()=>{continuousAudit.toggle();paint();};
 const mode=root?.querySelector<HTMLInputElement>('[data-quick-audit-market]');if(mode)mode.onchange=()=>{continuousAudit.mode(mode.checked);paint();};
 const refresh=root?.querySelector<HTMLButtonElement>('[data-learning-refresh]');
 if(refresh)refresh.onclick=()=>void load();
 const select=root?.querySelector<HTMLSelectElement>('[data-learning-filter]');
 if(select)select.onchange=()=>{filter=select.value;generation++;busy=false;data=undefined;void load();};
 const market=root?.querySelector<HTMLButtonElement>('[data-learning-market]');
 if(market)market.onclick=()=>document.querySelector<HTMLButtonElement>('[data-view="live"]')?.click();
}
export function bindLearning(){
 const next=document.querySelector('#learning-workspace');
 if(next!==root){if(timer)clearInterval(timer);timer=undefined;generation++;busy=false;root=next;
  if(root){void load();timer=setInterval(()=>{if(!document.hidden&&root?.isConnected)void load();},5000);}}
 wire();
}

