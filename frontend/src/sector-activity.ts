import {escapeHtml as esc} from './model';
import {interpretationFresh} from './confirmations';
import type {Scanner} from './scanner';
import './sector-activity.css';
export interface SectorActivity {rotation?:Rotation[];barEndMs:number|null;sectors:{name:string;expected:number;observed:number;notional:number|null;sharePct:number|null;aboveVwapPct:number|null;vwapObserved:number;meanRvol:number|null;rvolObserved:number;leaders:string[]}[]}
const amount=new Intl.NumberFormat('es',{notation:'compact',maximumFractionDigits:1});
const clock=new Intl.DateTimeFormat('es',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit'});
export interface RotationLeader {symbol:string;sector:string;currentUsd:number;previousUsd:number;changeUsd:number}
export interface Rotation {leaders?:RotationLeader[];minutes:number;ready:boolean;reason:string;coveragePct:number;sectors:{name:string;currentUsd:number;previousUsd:number;changeUsd:number;sharePct:number;changePp:number;observed:number;expected:number;state:string}[]}
function rotationPanel(rotations:Rotation[]|undefined,fresh:boolean):string {
 const signed=(v:number,suffix:string)=>`${v>0?'+':''}${v.toFixed(1)}${suffix}`;
 return `<div class="rotation-panels">${(rotations??[]).map(r=>{
  const ready=fresh&&r.ready;
  const gains=ready?r.sectors.filter(s=>s.state==='GANANDO CONCENTRACIÓN'):[];
  const losses=ready?r.sectors.filter(s=>s.state==='PERDIENDO CONCENTRACIÓN'):[];
  const names=(rows:Rotation['sectors'])=>rows.map(s=>esc(s.name.replaceAll('_',' '))).join(' · ');
  return `<section class="rotation-window"><header><h3>Rotación estimada · ${r.minutes} min</h3><small>Cobertura comparable: ${fresh?r.coveragePct.toFixed(0):'—'}%</small></header><p>${!ready?'Esperando historial comparable':gains.length&&losses.length?`<span class="rotation-loss rotation-signal">↓ Pierden: ${names(losses)}</span><span class="rotation-gain rotation-signal">↑ Ganan: ${names(gains)}</span>`:'Sin rotación conjunta clara entre sectores'}</p><small>${fresh?esc(r.reason):'Sin datos vigentes'}</small>${ready?`<div class="table-wrap"><table><thead><tr><th>Sector / estado</th><th>Concentración</th><th>Cambio</th><th>Nocional aprox. USD</th><th>Δ actividad USD</th></tr></thead><tbody>${r.sectors.map(s=>`<tr><td>${esc(s.name.replaceAll('_',' '))}<small class="exposure-dates">${esc(s.state)} · ${s.observed}/${s.expected}</small></td><td>${s.sharePct.toFixed(1)}%</td><td class="${s.state==='GANANDO CONCENTRACIÓN'?'rotation-gain':s.state==='PERDIENDO CONCENTRACIÓN'?'rotation-loss':'rotation-neutral'}">${signed(s.changePp,' pp')}</td><td>${amount.format(s.currentUsd)}<small class="exposure-dates">Antes: ${amount.format(s.previousUsd)}</small></td><td>${s.changeUsd>0?'+':''}${amount.format(s.changeUsd)}</td></tr>`).join('')}</tbody></table></div>`:''}</section>`;
 }).join('')}</div><p class="scanner-note">Compara los últimos 10 o 30 minutos con los 10 o 30 anteriores. Gana/pierde requiere cambio de al menos 1 punto porcentual y aumento/disminución de actividad en USD. Es una señal de redistribución de actividad, no flujo neto de capital ni presión compradora/vendedora confirmada.</p>`;
}
export function sectorActivityPanel(data:Scanner|undefined,active:boolean,published:number|undefined,now=Date.now()):string {
 const report=data?.sectorActivity,end=report?.barEndMs;
 const fresh=!!(active&&data?.connected&&interpretationFresh(published,now)&&end!=null&&end<=now&&now-end<=120000);
 const value=(n:number|null|undefined,suffix='')=>n!=null&&Number.isFinite(n)?`${n.toFixed(1)}${suffix}`:'—';
 return `<section class="sector-radar panel" aria-label="Radar de sectores independiente"><div class="panel-head"><div><span class="eyebrow">ACTIVIDAD SECTORIAL · VENTANA 2M</span><h2>Radar de sectores</h2><p class="sector-intro">Todos los sectores · concentración y rotación estimada de actividad</p></div><span class="tag">${fresh?`Corte ${clock.format(end!)} ET`:'Esperando sesión y datos vigentes'}</span></div>${rotationPanel(report?.rotation,fresh)}<div class="sector-grid">${(report?.sectors??[]).map(s=>{
  const valid=fresh&&s.observed>0, breadth=valid?s.aboveVwapPct:null;
  const tone=breadth==null?'neutral':breadth>50?'positive':breadth<50?'negative':'neutral';
  return `<article class="sector-tile ${tone}"><header><h3>${esc(s.name.replaceAll('_',' '))}</h3><small>${valid?s.observed:0}/${s.expected} activos</small></header><strong>${valid?value(s.sharePct,'%'):'—'}<small>de la actividad observada</small></strong><div class="sector-bar"><i style="width:${valid&&Number.isFinite(s.sharePct)?Math.max(0,Math.min(100,s.sharePct!)):0}%"></i></div><dl><div><dt>Nocional aprox. USD</dt><dd>${valid&&s.notional!=null?amount.format(s.notional):'—'}</dd></div><div><dt>RVOL medio · ${valid?s.rvolObserved:0} activos</dt><dd>${valid?value(s.meanRvol,'×'):'—'}</dd></div><div><dt>Sobre VWAP · ${valid?s.vwapObserved:0} activos</dt><dd>${value(breadth,'%')}</dd></div></dl><footer>${valid?`Líderes por actividad: ${s.leaders.map(esc).join(' · ')}`:'Sin datos suficientes'}</footer></article>`;
 }).join('')||'<p>Conecta Massive en Mercado en vivo para cargar el radar.</p>'}</div><p class="scanner-note">Cobertura del universo configurado, excluyendo ETF e índices. Orden por precio × volumen de la misma vela de 2 minutos; puede favorecer sectores con más acciones. Verde/rojo indica amplitud sobre/bajo VWAP. No mide entradas netas, compras institucionales ni transferencias identificadas entre sectores. Cobertura parcial = clasificación provisional.</p></section>`;
}
