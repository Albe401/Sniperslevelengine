import {legacyJournal,bindLegacy} from './legacy-journal';
import {invoke,isTauri} from './snapshot-observation';
import {escapeHtml as esc,price} from './model';

export interface PaperTrade {side:string;entry:number;current:number;stop:number;target1:number;target2:number|null;mfe:number;mae:number;pnlPoints:number;pnlPercent:number;stage:string;openedMs:number;closedMs:number|null;exitReason:string|null}
export interface JournalReport {
 page:{source:string;entries:{id:number;session:string;symbol:string;trade:PaperTrade}[];nextBeforeId:number|null};
 performance:{total_trades:number;wins:number;losses:number;win_rate:number;average_pnl_points:number;average_mfe:number;average_mae:number;target_1_hit_rate:number;target_2_hit_rate:number;stop_rate:number};
 legacyArchivedTrades:number;entriesEnabled:boolean;
}
let report:JournalReport|undefined;
let error='';
let loading=false;
let generation=0;
const time=(ms:number)=>new Date(ms).toLocaleTimeString('es-US',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit',second:'2-digit',hour12:false});
export function journalContent(data:JournalReport|undefined,busy:boolean,message:string):string {
 const stats=data?.performance;
 return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">SIMULACIONES LOCALES · RUST</span><h2>Diario de operaciones</h2></div><button class="replay-button" data-journal="refresh" ${busy?'disabled':''}>${busy?'Consultando…':'Actualizar'}</button></div>
 <p class="panel-caption">Resultados en puntos del subyacente. Las entradas nuevas requieren habilitar el simulador al conectar. Las estadísticas corresponden a operaciones Rust; el archivo Python se consulta abajo.</p>
 ${message?`<p class="panel-caption" role="alert">${esc(message)}</p>`:''}
 <div class="metrics"><div class="metric"><span>Operaciones cerradas</span><div>${stats?.total_trades??'—'}</div><p>${stats?`${stats.wins} positivas · ${stats.losses} negativas`:'Sin consulta completada'}</p></div><div class="metric"><span>Tasa de acierto</span><div>${stats?.total_trades?`${price(stats.win_rate)} %`:'—'}</div><p>Las operaciones neutras cuentan en el total</p></div><div class="metric"><span>PnL medio · puntos</span><div>${stats?.total_trades?price(stats.average_pnl_points):'—'}</div><p>Solo posiciones cerradas</p></div><div class="metric"><span>MFE / MAE medios</span><div>${stats?.total_trades?`${price(stats.average_mfe)} / ${price(stats.average_mae)}`:'—'}</div><p>Excursión favorable / adversa · puntos</p></div></div>
 <div class="table-wrap"><table><thead><tr><th>SESIÓN / ACTIVO</th><th>LADO / ESTADO</th><th>ENTRADA / ACTUAL</th><th>STOP / ETAPA</th><th>OBJETIVOS</th><th>PnL · PUNTOS</th><th>MFE / MAE</th></tr></thead><tbody>${data?.page.entries.map(({session,symbol,trade:t})=>`<tr><td>${esc(session)}<small class="exposure-dates">${esc(symbol)} · ${time(t.openedMs)} ET</small></td><td>${esc(t.side)} · ${t.closedMs==null?'Abierta':'Cerrada'}<small class="exposure-dates">${esc(t.exitReason??'Seguimiento simulado')}</small></td><td>${price(t.entry)} / ${price(t.current)}</td><td>${price(t.stop)}<small class="exposure-dates">${esc(t.stage)}</small></td><td>${price(t.target1)} / ${t.target2==null?'—':price(t.target2)}</td><td>${price(t.pnlPoints)}<small class="exposure-dates">${t.closedMs==null?'Al último precio observado':'Resultado de cierre'}</small></td><td>${price(t.mfe)} / ${price(t.mae)}</td></tr>`).join('')||`<tr><td colspan="7">${busy?'Recuperando operaciones guardadas…':message?'Diario no disponible.':data?'Todavía no hay operaciones Rust guardadas.':'Abre esta vista en la aplicación de Windows para consultar el diario local.'}</td></tr>`}</tbody></table></div>
 ${data?`<p class="panel-caption">T1: ${stats?.total_trades?`${price(stats.target_1_hit_rate)} %`:'—'} · T2: ${stats?.total_trades?`${price(stats.target_2_hit_rate)} %`:'—'} · Stop: ${stats?.total_trades?`${price(stats.stop_rate)} %`:'—'}. ${data.legacyArchivedTrades} operaciones antiguas archivadas en este almacén, excluidas de estas estadísticas.</p>`:''}
 <div class="panel-head"><span class="muted">25 posiciones por página · más recientes primero</span><button class="replay-button" data-journal="next" ${busy||data?.page.nextBeforeId==null?'disabled':''}>Ver anteriores</button></div></section>`;
}
export const journalPage=()=>`<div id="paper-journal">${journalContent(report,loading,error)}${legacyJournal()}</div>`;
function paint(){const root=document.querySelector('#paper-journal');if(root){root.innerHTML=journalContent(report,loading,error)+legacyJournal();bindJournal();}}
async function load(beforeId:number|null){
 const token=++generation;loading=true;error='';report=undefined;paint();
 try { if(!isTauri())throw new Error('El diario local está disponible en la aplicación de Windows.');
  const result=await invoke<JournalReport>('get_paper_journal',{beforeId});
  if(token!==generation)return;report=result;
 } catch(reason){if(token!==generation)return;error=typeof reason==='string'?reason:reason instanceof Error?reason.message:'No se pudo recuperar el diario.';}
 finally{if(token===generation){loading=false;paint();}}
}
export function bindJournal(){bindLegacy();document.querySelectorAll<HTMLButtonElement>('[data-journal]').forEach(b=>b.onclick=()=>void load(b.dataset.journal==='next'?report?.page.nextBeforeId??null:null));}
export function openJournal(){void load(null);}
