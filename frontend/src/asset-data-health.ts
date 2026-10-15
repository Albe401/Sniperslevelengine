import type {LiveState} from './live';
import {escapeHtml as esc} from './model';
export function assetDataHealth(s:LiveState|undefined):string {
 const snap=s?.optionSnapshotStatus,history=s?.offExchange.history??{};
 const statuses=Object.entries(history);
 const ready=statuses.filter(([,h])=>h.phase==='ready').length;
 const loading=statuses.filter(([,h])=>h.phase==='loading');
 const failed=statuses.filter(([,h])=>h.phase==='failed');
 const detail=loading.map(([date,h])=>`${date}: ${h.pages} páginas · ${h.stockTrades.toLocaleString('es')} operaciones revisadas · ${h.offExchangeTrades.toLocaleString('es')} registros TRF recibidos`).join(' / ');
 return `<section class="panel asset-data-health" aria-label="Cobertura de datos"><strong>Estado de los datos</strong><p>Opciones: ${snap?.contracts??0} contratos recibidos · sin OI: ${snap?.oiMissing??0} · sin delta o gamma: ${snap?.greeksMissing??0}. ${esc(snap?.detail??'Esperando conexión.')}${snap?.exposureError?` ${esc(snap.exposureError)}`:''}</p><p>Dark pool / TRF: ${ready} / 6 fechas descargadas.${loading.length?` Descargando ${esc(detail)}. El histórico se incorpora al completar cada fecha; recibir páginas todavía no equivale a cero operaciones.`:!statuses.length?' Esperando descarga del historial.':''}</p>${failed.map(([date,h])=>`<p class="negative">${esc(date)}: ${esc(h.detail)} · reintento automático.</p>`).join('')}${s?.offExchange.error?`<p class="negative">TRF: ${esc(s.offExchange.error)}</p>`:''}</section>`;
}
