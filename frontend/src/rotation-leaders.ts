import {escapeHtml as esc,price} from './model';
import {interpretationFresh} from './confirmations';
import type {Scanner} from './scanner';
export function rotationLeaders(data:Scanner|undefined,active:boolean,published:number|undefined,now:number,minutes:number,sector='') {
 const report=data?.sectorActivity,end=report?.barEndMs;
 const window=report?.rotation?.find(r=>r.minutes===minutes);
 const fresh=!!(active&&data?.connected&&interpretationFresh(published,now)&&end!=null&&end<=now&&now-end<=120000);
 const ready=!!(fresh&&window?.ready&&window.coveragePct>=80);
 const gains=new Set(ready?window!.sectors.filter(s=>s.state==='GANANDO CONCENTRACIÓN').map(s=>s.name):[]);
 const paired=ready&&window!.sectors.some(s=>s.state==='PERDIENDO CONCENTRACIÓN')&&gains.size>0;
 const candidates=paired?(window?.leaders??[]).filter(r=>gains.has(r.sector)&&(!sector||sector===r.sector)&&Number.isFinite(r.currentUsd)&&r.currentUsd>0&&Number.isFinite(r.previousUsd)&&r.previousUsd>=0&&Number.isFinite(r.changeUsd)&&r.changeUsd>0):[];
 const rows=candidates.filter(r=>candidates.filter(x=>x.symbol===r.symbol).length===1).sort((a,b)=>b.changeUsd-a.changeUsd||b.currentUsd-a.currentUsd||a.symbol.localeCompare(b.symbol)).slice(0,5);
 return {rows,ready,paired,reason:!fresh?'Sin datos vigentes':!ready?window?.reason??'Acumulando ventanas comparables':!paired?'Sin rotación conjunta clara':rows.length?'Sectores receptores · aumento de actividad':'Sin activos con aumento verificado en este filtro'};
}
export function rotationCards(data:Scanner|undefined,result:ReturnType<typeof rotationLeaders>,minutes:number):string {
 const usd=new Intl.NumberFormat('es',{style:'currency',currency:'USD',notation:'compact',maximumFractionDigits:2});
 return Array.from({length:5},(_,i)=>{
  const r=result.rows[i];
  if(!r)return `<article class="scanner-card scanner-empty"><span class="scanner-position">0${i+1}</span><h3>Esperando activo</h3><p>Sin clasificación disponible</p><small>${esc(result.reason)}</small></article>`;
  const row=data?.rows.find(a=>a.symbol===r.symbol&&a.barEndMs===data.sectorActivity?.barEndMs);
  const direction=row?.price!=null&&row.vwap!=null?(row.price>row.vwap?'Sobre VWAP':row.price<row.vwap?'Bajo VWAP':'En VWAP'):'Dirección pendiente';
  return `<article class="scanner-card"><div class="scanner-card-head"><span class="scanner-position">0${i+1}</span><span class="scanner-change is-up">${i===0?'LÍDER':'EN VIGILANCIA'}</span></div><h3>${esc(r.symbol)}</h3><small>${esc(r.sector.replaceAll('_',' '))}</small><div class="scanner-volume">+${usd.format(r.changeUsd)}<small>aumento de actividad · ${minutes} min</small></div><dl class="scanner-evidence"><div><dt>USD actuales aprox.</dt><dd>${usd.format(r.currentUsd)}</dd></div><div><dt>Ventana anterior</dt><dd>${usd.format(r.previousUsd)}</dd></div><div><dt>Precio actual</dt><dd>${row?.price!=null?price(row.price):'—'}</dd></div><div><dt>RVOL actual</dt><dd>${row?.rvol!=null?row.rvol.toFixed(2)+'×':'—'}</dd></div></dl><p class="scanner-direction">${direction}</p><div class="scanner-verdict">Candidato por rotación<small>No confirma compra ni entrada</small></div></article>`;
 }).join('');
}
