import { escapeHtml as esc, price } from './model';
import { interpretationFresh } from './confirmations';
export interface SpxSynthetic {
  phase:string;detail:string;contracts:number;
  estimate:null|{source:string;approximate:boolean;usableForAnalysis:boolean;expiration:string;root:string;price:number;candidatePairs:number;pairs:{strike:number;synthetic:number}[];oldestQuoteMs:number};
}
const etDate=(ms:number)=>new Date(ms).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
export function spxUsable(status:SpxSynthetic|undefined,connected:boolean,publishedMs:number|undefined,now=Date.now()):boolean {
  const e=status?.estimate;
  if(!connected||!interpretationFresh(publishedMs,now)||status?.phase!=='ready'||!e)return false;
  const expiry=Date.parse(`${e.expiration}T00:00:00Z`),today=Date.parse(`${etDate(now)}T00:00:00Z`);
  return e.source==='LEGACY_PUT_CALL_PARITY_APPROXIMATION'&&e.approximate&&e.usableForAnalysis
    &&['SPX','SPXW'].includes(e.root)&&Number.isFinite(e.price)&&e.price>0
    &&e.pairs.length>=3&&e.pairs.length<=15&&e.candidatePairs>=e.pairs.length
    &&Number.isFinite(e.oldestQuoteMs)&&e.oldestQuoteMs<=now&&now-e.oldestQuoteMs<=15_000
    &&etDate(e.oldestQuoteMs)===etDate(now)&&/^\d{4}-\d{2}-\d{2}$/.test(e.expiration)
    &&Number.isFinite(expiry)&&new Date(expiry).toISOString().slice(0,10)===e.expiration
    &&expiry>=today&&expiry-today<=10*86_400_000;
}
export function spxSyntheticPanel(status:SpxSynthetic|undefined,connected:boolean,publishedMs:number|undefined,now=Date.now()):string {
  const usable=spxUsable(status,connected,publishedMs,now),e=status?.estimate;
  return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">PARIDAD CALL / PUT</span><h2>SPX sintético</h2></div><span class="tag">ESTIMACIÓN</span></div>
  <div class="table-wrap"><table><thead><tr><th>REFERENCIA APROXIMADA</th><th>CONTRATOS</th><th>VENCIMIENTO</th><th>PARES VÁLIDOS</th></tr></thead><tbody><tr><td>${usable&&e?`≈ ${price(e.price)}`:'—'}</td><td>${usable&&e?esc(e.root):'—'}</td><td>${usable&&e?esc(e.expiration):'—'}</td><td>${usable&&e?e.pairs.length:'—'}</td></tr></tbody></table></div>
  <div class="panel-caption">${!connected?'Sin conexión vigente.':!interpretationFresh(publishedMs,now)?'Publicación pendiente o caducada.':usable?'Estimación disponible.':esc(status?.detail??'Esperando pares de opciones SPX.')} Aproximación K + Call − Put, sin ajuste de dividendos ni financiación. No es un tick directo del índice. Usa cotizaciones WebSocket recientes de la misma raíz y vencimiento; SPX y SPXW se mantienen separados. No autoriza entradas.</div></section>`;
}
