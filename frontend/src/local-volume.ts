import {escapeHtml as esc,price} from './model';
import {interpretationFresh} from './confirmations';
export interface LocalVolume {symbol:string;state:string;usable:boolean;rvol_local:number|null;contracts_1m:number|null;baseline_contracts_per_minute:number|null;call_contracts_1m:number|null;put_contracts_1m:number|null;last_trade_age_seconds:number|null;reason:string}
export function localVolumeFresh(r:LocalVolume|undefined,symbol:string,active:boolean,published:number|undefined,now=Date.now()):boolean {
 return Boolean(active&&r?.symbol===symbol&&r.usable&&interpretationFresh(published,now)&&r.last_trade_age_seconds!=null&&r.last_trade_age_seconds>=0&&r.last_trade_age_seconds+(now-(published??now))/1000<=15);
}
export function localVolumePanel(r:LocalVolume|undefined,symbol:string,active:boolean,published:number|undefined,now=Date.now()):string{
 const usable=localVolumeFresh(r,symbol,active,published,now);
 const reason=!active||r?.symbol!==symbol?'Esperando flujo propio vigente.':!interpretationFresh(published,now)?'Publicación caducada; esperando una actualización.':r.usable&&!usable?'Operaciones caducadas; esperando flujo reciente.':r.reason;
 const number=(value:number|null|undefined)=>usable&&value!=null&&Number.isFinite(value)?price(value):'—';
 return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">OPCIONES · VENCIMIENTO DE HOY</span><h2>Ritmo local de volumen</h2></div><span class="tag">${usable?esc(r!.state):'ESPERANDO COBERTURA'}</span></div><div class="metrics"><div class="metric"><span>Ritmo 1m / base previa</span><div>${number(r?.rvol_local)}</div><p>Comparación local, no RVOL histórico</p></div><div class="metric"><span>Contratos del último minuto</span><div>${number(r?.contracts_1m)}</div><p>Contratos seleccionados del activo</p></div><div class="metric"><span>Base por minuto</span><div>${number(r?.baseline_contracts_per_minute)}</div><p>Promedio de los cuatro minutos previos</p></div><div class="metric"><span>CALL / PUT clasificados</span><div>${number(r?.call_contracts_1m)} / ${number(r?.put_contracts_1m)}</div><p>Solo operaciones con lado inferido</p></div></div><p class="panel-caption">${esc(reason)} Esta lectura no modifica las puntuaciones ni autoriza entradas. Una rotación o desconexión requiere reconstruir la cobertura.</p></section>`;
}
