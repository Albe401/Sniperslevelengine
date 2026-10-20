import type {LiveState} from './live';
import {currentMapHorizon} from './reading-display';
import {escapeHtml as esc} from './model';

/** Presentation of an archived observation, never permission for a live signal. */
export function savedReadingDisplay(frame:LiveState,symbol:string,capturedMs:number,clock:number){
 const valid=frame.symbol===symbol&&Number.isSafeInteger(capturedMs)&&capturedMs>0&&Number.isFinite(clock)&&capturedMs<=clock&&Number.isFinite(frame.publishedMs)&&frame.publishedMs>0&&frame.publishedMs<=capturedMs;
 const horizon=valid?currentMapHorizon(frame.optionSnapshotStatus,capturedMs):undefined;
 const current=valid&&clock-capturedMs<=15*60_000&&!!currentMapHorizon(frame.optionSnapshotStatus,clock);
 const partial=!!horizon&&(!horizon.structure.complete||horizon.structure.liquidityComplete===false);
 const p=horizon?.structure.provisional;
 const coverage=p&&Number.isSafeInteger(p.coveredContracts)&&Number.isSafeInteger(p.totalContracts)&&p.coveredContracts>0&&p.totalContracts>=p.coveredContracts?`${p.coveredContracts}/${p.totalContracts} contratos incluidos`:'';
 return {valid,current,partial,coverage,renderAt:capturedMs,label:!valid?'Captura inválida':current?'Captura reciente': 'Referencia histórica'};
}

export function savedReadingNotice(frame:LiveState,symbol:string,capturedMs:number,clock:number){
 const state=savedReadingDisplay(frame,symbol,capturedMs,clock);
 if(!state.valid)return '<section class="panel" role="status">Captura inválida. Esperando una observación identificada del activo.</section>';
 const at=new Date(capturedMs).toLocaleString('es',{timeZone:'America/New_York'});
 return `<section class="panel web-saved-reading-notice" role="status" data-saved-reading-state="${state.current?'recent':'historical'}"><strong>${state.label}${state.partial?' · cobertura parcial':''} · ${esc(symbol)}</strong><p>Captura ${esc(at)} ET.${state.coverage?' '+state.coverage+'.':''} Lectura calculada al momento de la captura; no es una señal vigente ni datos en tiempo real. Los campos sin datos permanecen pendientes.</p></section>`;
}
