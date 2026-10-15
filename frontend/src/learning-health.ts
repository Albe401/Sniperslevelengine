import type {LearningReport} from './learning';
import {escapeHtml as esc} from './model';
export function learningHealthPanel(d:LearningReport|undefined,now:number){
 const rows:{name:string;state:string;detail:string}[]=[];
 if(!d)return '<section class="learn-panel"><h2>Revisión del bot y del sistema</h2><p>Esperando consulta real del registro.</p></section>';
 const fresh=now>=d.generatedMs&&now-d.generatedMs<=75000;
 rows.push({name:'Consulta del sistema',state:fresh?'✓ Vigente':'⚠ Caducada',detail:d.connected?'Mercado conectado':'Mercado desconectado; se muestra la memoria guardada.'});
 const b=d.bot;
 rows.push({name:'Operaciones del bot',state:!b?.available?'⚠ Sin consulta':b.incomplete?'⚠ Revisar incompletas':'✓ Diario consultado',detail:b?.available?`${b.closed} cerradas verificadas · ${b.open} abiertas · ${b.pending} pendientes · ${b.incomplete} incompletas`:'No se pudo verificar el diario.'});
 const a=b?.journalAudit;
 rows.push({name:'Decisiones guardadas · todos los activos',state:!a?.available?'○ Sin verificación':a.future||a.invalidJson||a.orphaned?'⚠ Revisar registro':'✓ Sin anomalías en estas comprobaciones',detail:a?.available?`${a.records} decisiones · ${a.future} tiempos futuros · ${a.invalidJson} registros inválidos · ${a.orphaned} sin sesión asociada`:'Falta consultar las decisiones.'});
 const c=d.collector,valid=!!c&&c.recorded&&now>=c.observed_ms&&now-c.observed_ms<=75000;
 rows.push({name:'Registro de aprendizaje',state:valid?'✓ Guardando':'○ Sin registro reciente',detail:c?`${c.symbol} · ${c.quality}${c.eligible?' · apta para evaluar':' · datos incompletos o no elegibles'}`:'Esperando datos.'});
 rows.push({name:'Calidad del historial',state:d.eligible<d.observations?'⚠ Hay lecturas excluidas':'✓ Conteo consultado',detail:`${d.eligible} aptas de ${d.observations}; las incompletas no validan una mejora.`});
 return `<section class="learn-panel"><h2>Revisión del bot y del sistema</h2><p>Actualiza al consultar Aprendizaje. Revisa registros y calidad disponible; no certifica todas las reglas ni la rentabilidad.</p><div class="learn-table-scroll"><table><thead><tr><th>Comprobación</th><th>Estado</th><th>Evidencia</th></tr></thead><tbody>${rows.map(r=>`<tr><td>${esc(r.name)}</td><td>${esc(r.state)}</td><td>${esc(r.detail)}</td></tr>`).join('')}</tbody></table></div></section>`;
}
