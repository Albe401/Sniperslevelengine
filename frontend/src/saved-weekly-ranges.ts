import {scannedWeeklyEntry,weeklyRange,type WeeklySnapshot} from './investment-weekly-ranges';
import {escapeHtml as esc} from './model';
import type {EtfReport} from './etf-dealers';
export function savedWeeklyRanges(symbol:string,report:EtfReport|undefined,capturedMs:number){
 const heading=`<h2>Radar de inversión · Rangos semanales · ${esc(symbol)}</h2>`;
 try{
  if(!report||!Number.isFinite(report.at)||report.at>capturedMs)throw Error('Cadena de opciones pendiente en esta captura.');
  // Reuse the desktop reader and its input checks at the original observation time.
  // Inventory spans six months; the reader selects only expiries within seven days.
  const entry=scannedWeeklyEntry({...report,days:7} as WeeklySnapshot,symbol,report.at);
  const range=weeklyRange(entry.price,entry.iv,entry.days);
  if(!range)throw Error('Datos insuficientes para un rango válido.');
  const money=(n:number)=>'$'+n.toFixed(2);
  return `<section class="panel investment-weekly-ranges">${heading}<p>Estimación guardada · ${esc(new Date(report.at).toLocaleString('es',{timeZone:'America/New_York'}))} ET. No es una lectura en vivo.</p><article class="weekly-range-card"><header><h2>${esc(symbol)}</h2><span>Vencimiento ${esc(entry.expiry)} ET</span></header><div class="weekly-range-meta"><span>IV anualizada ${entry.iv.toFixed(2)}%</span><strong>Movimiento ±${money(range.move)} (${range.percent.toFixed(2)}%)</strong></div><div class="weekly-range-levels">${[['weekly-lower','Límite inferior',range.lower],['weekly-base','Referencia',entry.price],['weekly-upper','Límite superior',range.upper]].map(([cls,label,value])=>`<div class="${cls}"><span>${label}</span><b>${money(value as number)}</b><i></i></div>`).join('')}</div><footer>${entry.days.toFixed(2)} días calendario · ${esc(entry.source??'Cadena guardada de Massive')}</footer></article><p>Movimiento estimado = precio × IV anualizada × √(días / 365). Modelo de volatilidad; no es una garantía ni una probabilidad calibrada. Se conserva el cálculo del escritorio.</p></section>`;
 }catch(error){return `<section class="panel investment-weekly-ranges">${heading}<p>Rango pendiente: ${esc(error instanceof Error?error.message:'Datos inválidos.')}</p><p>Se requieren precio y par CALL/PUT ATM con IV, fechas y cotizaciones válidas de la misma observación. No se sustituyen por cifras de ejemplo.</p></section>`;}
}
