import {escapeHtml as esc,price} from './model';
import {spxUsable,type SpxSynthetic} from './spx-synthetic';
import type {Horizon} from './live';
export interface SpxReading {
 phase:string;detail:string;observedMs:number|null;referenceMs:number|null;spot:number|null;
 referenceRoot:string;referenceExpiration:string;contracts:number;excludedAmContracts:number;horizons:Horizon[];entryConfirmed:boolean;oiLevels:{label:string;callStrike:number|null;putStrike:number|null;callOi:number|null;putOi:number|null;known:number;total:number}[];
}
export function spxReadingFresh(r:SpxReading|undefined,s:SpxSynthetic|undefined,connected:boolean,publishedMs:number|undefined,now=Date.now()):boolean {
 const fresh=(t:number|null)=>t!=null&&Number.isFinite(t)&&t<=now&&now-t<=300_000&&new Date(t).toLocaleDateString('en-CA',{timeZone:'America/New_York'})===new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
 return !!r&&r.phase==='ready'&&spxUsable(s,connected,publishedMs,now)&&fresh(r.observedMs)&&fresh(r.referenceMs)
  &&r.referenceRoot===s?.estimate?.root&&r.referenceExpiration===s?.estimate?.expiration
  &&r.spot!=null&&Number.isFinite(r.spot)&&r.spot>0&&!r.entryConfirmed;
}
export function spxReadingPanel(r:SpxReading|undefined,s:SpxSynthetic|undefined,connected:boolean,publishedMs:number|undefined,now=Date.now()):string {
 const valid=spxReadingFresh(r,s,connected,publishedMs,now),horizons=valid?r!.horizons:[];
 const n=(x:number|null|undefined)=>x==null||!Number.isFinite(x)?'—':price(x);
 const compact=(x:number)=>Intl.NumberFormat('es',{notation:'compact',maximumFractionDigits:2}).format(x);
 return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">SPXW · LIQUIDACIÓN PM · SOLO LECTURA</span><h2>Niveles y exposición de opciones SPX</h2></div><span class="tag">${valid?'REFERENCIA APROXIMADA':'ESPERANDO DATOS'}</span></div>
 <p>${esc(valid?r!.detail:r?.phase==='waiting'&&r.detail?r.detail:'Conecta Data y espera cotizaciones SPX recientes y una cadena vigente.')}</p>
 ${valid?`<p>${r!.contracts} contratos PM en el rango local · ${r!.excludedAmContracts} contratos AM separados · spot de cálculo ≈ ${n(r!.spot)} · snapshot ${new Date(r!.observedMs!).toLocaleTimeString('es',{timeZone:'America/New_York'})} ET.</p>`:''}
 ${valid?`<h3>Concentraciones de interés abierto</h3><p>Máximos OI observados en el rango; no son muros confirmados ni señales. Disponibles incluso cuando faltan griegas.</p><div class="table-wrap"><table><thead><tr><th>HORIZONTE</th><th>STRIKE CALL / OI</th><th>STRIKE PUT / OI</th><th>COBERTURA OI</th></tr></thead><tbody>${(r!.oiLevels??[]).map(o=>`<tr><td>${esc(o.label)}</td><td>${n(o.callStrike)} / ${n(o.callOi)}</td><td>${n(o.putStrike)} / ${n(o.putOi)}</td><td>${o.known} de ${o.total}</td></tr>`).join('')}</tbody></table></div>`:''}
 <div class="table-wrap"><table><thead><tr><th>HORIZONTE</th><th>CALL WALL</th><th>PUT WALL</th><th>IMÁN</th><th>GEX ESTIMADO</th><th>DEX ESTIMADO</th><th>COBERTURA G / D</th></tr></thead><tbody>${horizons.length?horizons.map(h=>`<tr><td>${esc(h.label)}<small>${esc(h.expirations.join(', '))}</small></td><td>${n(h.structure.callWall?.strike)}</td><td>${n(h.structure.putWall?.strike)}</td><td>${n(h.structure.magnet)}</td><td>${h.exposure.gammaUsed?compact(h.exposure.netGex):'—'}${h.exposure.gexComplete?'':' · parcial'}</td><td>${h.exposure.deltaUsed?compact(h.exposure.netDex):'—'}${h.exposure.dexComplete?'':' · parcial'}</td><td>${h.exposure.gammaUsed} / ${h.exposure.deltaUsed} de ${h.exposure.eligible}</td></tr>`).join(''):'<tr><td colspan="7">La lectura se retira si pierde conexión, referencia o vigencia.</td></tr>'}</tbody></table></div>
 <div class="panel-caption">OI de observación diaria; griegas ausentes permanecen desconocidas. Los niveles completos requieren cobertura; no se convierten los resultados parciales en señales. La consulta cubre el rango configurado y próximos cuatro vencimientos más los de la semana, no toda la cadena.<br>Precio directo e histórico I:SPX: acceso rechazado por el proveedor en la comprobación del 27/09/2026. Esta vista no tiene velas oficiales, VWAP, RVOL ni flujo direccional confirmado del índice. No autoriza entradas ni modifica Snipers Control.</div></section>`;
}
