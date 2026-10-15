import {escapeHtml as esc} from './model';
export interface AnalystContext {symbol:string;snapshot_generated_at_ms:number;data_quality:{evidence_timestamp_ms:number;stocks_phase:string;options_phase:string;snapshot_phase:string};decision_runtime:{decision:{reason?:string;entry_confirmed?:boolean}}}
export function analystPanel(context:AnalystContext|undefined,active:boolean,available:boolean,busy:boolean,error:string,now=Date.now()):string {
  const fresh=active&&context!=null&&context.snapshot_generated_at_ms<=now&&now-context.snapshot_generated_at_ms<=15_000&&context.data_quality.evidence_timestamp_ms<=now&&now-context.data_quality.evidence_timestamp_ms<=15_000;
  return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">LECTURA ASISTIDA · OPCIONAL</span><h2>Analista</h2></div><span class="tag">PREPARACIÓN LOCAL</span></div>
  <div class="panel-caption">El contexto reúne estructura, flujo, scanner y bloqueos del coordinador. La conexión con IA está deshabilitada; esta revisión se realiza en tu equipo.</div>
  <div class="panel-caption"><button class="replay-button" data-analyst-context ${!active||!available||busy?'disabled':''}>${busy?'Preparando…':'Revisar contexto local'}</button></div>
  ${error?`<p class="panel-caption">${esc(error)}</p>`:''}
  ${context?`<div class="panel-caption"><strong>${fresh?'Contexto preparado': 'Contexto sin vigencia'} · ${esc(context.symbol)}</strong>${fresh?`<br>Opciones: ${esc(context.data_quality.options_phase)} · Estructura: ${esc(context.data_quality.snapshot_phase||'pendiente')}<br>Coordinador: ${esc(context.decision_runtime.decision.reason??'Sin revisión vigente')}`:'<br>Actualiza la revisión para utilizar datos recientes.'}</div>`:''}
  <div class="panel-caption">La narración del analista es informativa. Las entradas simuladas dependen exclusivamente de las validaciones del motor y de su habilitación explícita.</div></section>`;
}
