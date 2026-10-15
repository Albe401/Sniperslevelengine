import {atrMeter} from './atr-meter';
import {atrEvidence} from './atr-context';
import type {ControlReading} from './intraday-control';
import {escapeHtml as esc} from './model';
import {strikeAbsorptionRows,strikeAbsorptionDetails} from './strike-absorption';
import {volatilityScales} from './volatility-scales';
export function absorptionReading(r:ControlReading,now=Date.now()) {
 const p=r.priceNow,f=r.flowControl,v=r.participation;
 const ready=p?.ready&&p.spot!=null&&Number.isFinite(p.spot)&&p.spot>0&&(r.marketEvidence?r.marketEvidence.flow.signal!=null:f?.quality==='ready'&&!f.headline.includes('PARCIAL'));
 const volume=!!v&&Number.isFinite(v.rvol)&&v.rvol>=1&&v.at<=now&&now-v.at<=180000;
 const side=r.marketEvidence?(r.marketEvidence.flow.signal==null?0:Math.sign(r.marketEvidence.flow.signal)):f?.tone==='sellers'?-1:f?.tone==='buyers'?1:0;
 const events=ready?r.intradayMap?.events.filter(e=>e.sustained&&e.at<=now&&now-e.at<=180000&&r.intradayMap?.levels.some(l=>l.name===e.name&&l.level===e.level)&&Math.abs(e.level-p.spot!)/p.spot!<=.0035)??[]:[];
 const held=events.find(e=>side!==0&&e.direction===-side&&p!.direction===e.direction);
 const kind=held&&volume?(held.direction>0?'fall':'rise'):'waiting';
 const headline=kind==='fall'?'POSIBLE ABSORCIÓN DE LA CAÍDA':kind==='rise'?'POSIBLE ABSORCIÓN DE LA SUBIDA':'SIN INDICIOS SUFICIENTES';
 const detail=kind!=='waiting'?`${held!.name} $${held!.level.toFixed(2)}: ${held!.direction>0?'el precio sostiene una defensa pese al flujo de opciones vendedor':'el precio sostiene un rechazo pese al flujo de opciones comprador'}. RVOL ${v!.rvol.toFixed(2)}×. Divergencia observada; no confirma absorción en el activo.`:!ready?'Falta precio reciente, reacción persistente o flujo clasificado.':held&&!volume?'Hay divergencia entre precio y opciones; falta RVOL vigente ≥1 para destacar el indicio.':'Esperando defensa o rechazo persistente cerca del nivel, contra la presión del flujo y con participación relativa.';
 const invalidation=kind==='fall'?`El indicio deja de ser válido si el precio pierde $${held!.level.toFixed(2)}, desaparece la defensa o caduca la evidencia.`:kind==='rise'?`El indicio deja de ser válido si el precio recupera $${held!.level.toFixed(2)}, desaparece el rechazo o caduca la evidencia.`:'La lectura se reevalúa con cada actualización; no conserva señales caducadas.';
 return {level:held?.level,name:held?.name,vanna:r.vannaEngine&&r.vannaEngine.symbol===r.symbol&&r.vannaEngine.at===now?r.vannaEngine.absorption:undefined,kind,headline,detail,invalidation};
}
export function absorptionPanel(r:ControlReading,now=Date.now()):string {
 const a=absorptionReading(r,now);
 const scales=volatilityScales(r,now);
 return `<section class="absorption-reading absorption-${a.kind}" aria-label="Absorción de caídas y subidas"><details class="absorption-compact" data-horizon-details="absorption-evidence"><summary title="Ver strikes, fuerza, absorción y condición ATR"><span>DELTA POR STRIKE</span><strong>${a.headline}</strong><span class="absorption-expected-range" title="${esc(scales.detail)}">${esc(scales.label)}</span></summary><div class="absorption-compact-detail"><p>${esc(a.detail)}</p><p>${esc(scales.detail)}</p>${strikeAbsorptionRows(r)}<p class="atr-inline-summary">Condición · ${esc(r.atr?.context?.label??'límites')} ${atrMeter(r.atr)}</p>${atrEvidence(r.atr)}${strikeAbsorptionDetails(r)}<p>${esc(a.invalidation)}</p><p>Delta: sensibilidad de opciones del mismo strike y vencimiento, tomada del snapshot; no compras nuevas ni inventario confirmado de dealers. El cambio de fuerza procede del modelo de niveles, no del aumento de delta. Perder fuerza no confirma un giro.</p><p>Indicio heurístico: reacción persistente en un nivel a ≤0,35% del precio, flujo de opciones contrario y RVOL ≥1. No disponemos aquí de confirmación mediante volumen agresor y liquidez pasiva del activo; también puede ser una divergencia temporal. No autoriza entradas.</p></div></details></section>`;
}

