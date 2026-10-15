import {escapeHtml as esc,price} from './model';
import {interpretationFresh} from './confirmations';
import {preparationReasons} from './decision-review';
export interface StructuralPath {
 symbol:string;horizon:string;magnet:number|null;targetWall:number|null;vacuumLow:number|null;vacuumHigh:number|null;
 preparationReason?:string;simulationCompatible?:boolean;
 trigger:{ready:boolean;reason:string;volume_ratio:number|null;reference_price:number|null;bar_end_ms:number|null;vwap_supportive:boolean};
 candidate:{available:boolean;ready:boolean;side:string;invalidation_level:number|null;risk_reward:number|null;flow_ready:boolean;participation_ready:boolean;barrier_blocked:boolean;blocks:string[];entry_authorized:boolean};
}
const reasons:Record<string,string>={UNVERIFIED_INPUT:'Datos incompletos o sin vigencia',MISSING_GEOMETRY:'Faltan niveles',INCOMPLETE_MAGNET_VACUUM_WALL:'Geometría o fuerza del muro insuficiente',INVALID_STOP:'Invalidación no válida',MAGNET_2M_TRIGGER:'Falta confirmación del imán en 2m',LIVE_FLOW:'Flujo de opciones sin confirmación',PARTICIPATION:'Volumen insuficiente',OPTION_LIQUIDITY:'Liquidez de opciones pendiente',INTERMEDIATE_BARRIER:'Barrera fuerte en el recorrido',POSITION_CONFLICT:'Conflicto con el mapa de posición',RISK_REWARD:'Relación riesgo/beneficio inferior a 1,8'};
export function structuralPathPanel(rows:StructuralPath[]|undefined,active:boolean,published:number|undefined,now=Date.now()):string{
 const fresh=active&&interpretationFresh(published,now);
 const val=(v:number|null)=>v==null?'—':price(v);
 return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">RUTA ESTRUCTURAL · ACTIVO SELECCIONADO</span><h2>Imán → Vacío → Wall</h2></div><span class="tag">ANÁLISIS · ENTRADAS DESHABILITADAS</span></div>
 <div class="table-wrap"><table><thead><tr><th>ESCENARIO</th><th>CONDICIONES DEL MODELO</th><th>IMÁN / VACÍO / WALL</th><th>CONFIRMACIÓN 2M</th><th>INVALIDACIÓN / R:B</th></tr></thead><tbody>${(rows??[]).map(r=>{
  const c=r.candidate,t=r.trigger,valid=fresh&&!c.blocks.includes('UNVERIFIED_INPUT');
  const recent=valid&&t.bar_end_ms!=null&&t.bar_end_ms<=now&&now-t.bar_end_ms<=180_000;
  return `<tr><td>${esc(r.symbol)} · ${esc(c.side)}<small class="exposure-dates">${esc(r.horizon)}</small></td><td class="${recent&&c.ready?'structural-ready':'structural-wait'}">${recent&&c.ready?'Condiciones alineadas':'Esperar'}<small class="exposure-dates">${valid?c.blocks.map(b=>esc(reasons[b]??b)).join(' · ')||esc(preparationReasons[r.preparationReason??'']??'Pendiente del coordinador de sesión'):'Sin evidencia vigente'}</small></td><td>${valid?`${val(r.magnet)} / ${val(r.vacuumLow)}–${val(r.vacuumHigh)} / ${val(r.targetWall)}`:'—'}</td><td>${recent?esc(t.reason):'Esperando vela vigente'}<small class="exposure-dates">${recent?`Volumen ${val(t.volume_ratio)}x · VWAP ${t.vwap_supportive?'acompaña':'no acompaña'}`:'Sin confirmación'}</small></td><td>${recent?`${val(c.invalidation_level)} / ${val(c.risk_reward)}`:'—'}</td></tr>`;
 }).join('')||'<tr><td colspan="5">Esperando imán principal, muros, velas y opciones del activo.</td></tr>'}</tbody></table></div>
 <p class="panel-caption">El imán principal es M1. Se exige un vacío en el recorrido hacia un muro fuerte, confirmación de vela cerrada de 2 minutos, flujo comprador del lado correspondiente y liquidez vigente. Volumen ≥1x; RVOL suficiente o volumen ≥1,15x. Distancia al imán ≤0,35 % y riesgo/beneficio ≥1,8. Una barrera fuerte intermedia bloquea. VWAP aporta contexto. La ruta todavía no abre operaciones.</p></section>`;
}
