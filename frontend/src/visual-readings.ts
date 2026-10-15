import {displayStructure} from './reading-display';
import {escapeHtml as esc,price} from './model';
import type {Horizon} from './live';
import type {Alignment} from './reading';

const number=(n:number|null|undefined)=>n!=null&&Number.isFinite(n)?price(n):'—';
export function compactStructuralHorizons(horizons:Horizon[]|undefined,fresh:boolean):string {
 const rows=[['DAY','Un día'],['WEEK','Una semana'],['NEXT4','4 expiraciones']].map(([mode,label])=>{
  const h=horizons?.find(h=>h.mode===mode),r=h?.structure?.reading;
  const side=fresh&&h?.structure?.complete&&r?.available&&['CALL','PUT','NEUTRAL'].includes(r.side)?r.side:'Pendiente';
  return {label,side};
 });
 const sides=rows.map(r=>r.side),ready=sides.every(s=>s!=='Pendiente');
 const aligned=ready&&sides.every(s=>s===sides[0]);
 const status=!ready?'Alineación pendiente':aligned?`Alineados estructuralmente · ${sides[0]}`:'Sin alineación estructural';
 return `<section class="compact-structure" aria-label="Alineación estructural"><div class="compact-structure-grid">${rows.map(r=>`<div class="compact-structure-card"><span>${r.label}</span><strong class="${r.side==='CALL'?'positive':r.side==='PUT'?'negative':'amber-text'}">${r.side}</strong></div>`).join('')}</div><small role="status">${status}</small></section>`;
}
const metric=(label:string,n:number|null|undefined,tone='')=>`<div class="reading-number ${tone}"><span>${label}</span><strong>${number(n)}</strong></div>`;
export function assetAlignment(fresh:boolean,alignment?:Alignment|null,horizons?:Horizon[]):string {
 const a=fresh?alignment:undefined;
 const directional=a&&['FULL ALIGNMENT','PARTIAL ALIGNMENT'].includes(a.status)&&['CALL','PUT'].includes(a.side);
 const mixed=a&&['MIXED','NEUTRAL / MIXED'].includes(a.status);
 const label=directional?`ALINEADO ${a.side}${a.status==='PARTIAL ALIGNMENT'?' · PARCIAL':''}`:mixed?'MIXTO':a?.status==='NEUTRAL'?'NEUTRAL':a?'DATOS INSUFICIENTES':'ESPERANDO DATOS';
 const tone=directional?(a.side==='CALL'?'call':'put'):mixed||a?.status==='NEUTRAL'?'mixed':'waiting';
 const counts=a?`CALL ${a.callCount} · PUT ${a.putCount} · neutrales ${a.neutralCount} · sin lectura ${a.unavailableCount}. `:'';
 const pending=fresh?(horizons??[]).filter(h=>!h.structure?.complete||!h.structure?.reading?.available).map(h=>`${h.label}: ${h.structure?.reading?.reason||'Cobertura estructural incompleta'}`).join(' · '):'';
 const detail=!fresh?'Esperando conexión y un snapshot vigente del activo.':!a?'La alineación aún no está disponible en el snapshot.':pending||'Completa: tres horizontes en la misma dirección. Parcial: al menos dos, sin dirección contraria.';
 return `<div class="reading-alignment panel asset-alignment alignment-${tone}" role="status"><span>ALINEACIÓN DEL ACTIVO</span><h2>${label}</h2><p class="panel-caption">${esc(counts+detail)}</p></div>`;
}
export function horizonCards(horizons:Horizon[]|undefined,fresh:boolean,alignment?:Alignment|null):string {
  const rows=horizons?.length?horizons:[undefined,undefined,undefined];
  return `${assetAlignment(fresh,alignment,horizons)}<div class="horizon-grid">${rows.map((h,i)=>{
    const s=h?.structure,r=s?.reading,levelsValid=fresh&&s?.complete,valid=levelsValid&&r?.available;
    const display=displayStructure(h,fresh);
    const tone=valid&&r?.side==='CALL'?'positive':valid&&r?.side==='PUT'?'negative':'amber-text';
    return `<article class="panel horizon-card"><header><span class="eyebrow">${esc(h?.label??['Hoy / próxima sesión','Semana','Próximos 4 vencimientos'][i])}</span><h2 class="${tone}">${valid?esc(r.reading):display?'Referencias provisionales':'Esperando datos'}</h2><p class="panel-caption">Expiraciones: ${fresh&&h?esc(h.expirations.join(" / ")):"Pendientes"}</p></header><div class="reading-pair">${metric('Puntuación CALL / 100',valid?r.callScore:null,'positive')}${metric('Puntuación PUT / 100',valid?r.putScore:null,'negative')}</div><div class="reading-regime"><span>Régimen gamma</span><strong>${fresh&&h?.exposure.gexComplete?esc(h.exposure.gammaRegime):'Sin verificar'}</strong>${metric('Zero gamma',fresh&&h?.exposure.scanComplete?h.exposure.zeroGamma:null)}</div><div class="reading-pair">${metric('Imán principal',display?.selected?.M1,'purple-text')}${metric('CALL WALL',display?.callWall?.strike,'positive')}${metric('PUT WALL',display?.putWall?.strike,'negative')}${metric('Precio de referencia',fresh?h?.exposure.spot:null)}</div><p class="reading-reason">${fresh&&s?.provisional?`Cobertura de referencias: ${s.provisional.coveredContracts}/${s.provisional.totalContracts} contratos. `:''}${fresh?esc(r?.reason||'Estructura pendiente'):'Snapshot sin vigencia; esperando actualización.'}${fresh&&s&&!s.complete?` · ${s.missingContracts} contratos sin OI o volumen; ${s.rows} strikes disponibles.`:''}${fresh&&!h?.exposure.gexComplete?' · GEX incompleto: faltan griegas o cobertura requerida.':''}</p><div class="reading-routes">${(display?.routes??[]).map(route=>`<div><b class="${route.direction==='CALL'?'positive':'negative'}">Escenario ${esc(route.direction)}</b><p>${display?esc(route.pathState):'Sin verificar'}</p><span>Objetivo <strong>${number(display?route.target:null)}</strong> · Barrera <strong>${number(display?route.barrier:null)}</strong></span></div>`).join('')}</div><footer>Vencimientos: ${esc(h?.expirations.join(' · ')||'Pendientes')}</footer></article>`;
  }).join('')}</div>`;
}
export function quickLevels(horizons:Horizon[]|undefined,fresh:boolean,currentPrice:number|null|undefined=null):string {
  const h=horizons?.find(h=>h.mode==='DAY'),s=h?.structure;
  const valid=fresh&&s?.complete;
  return `<div class="quick-reading-grid"><section class="panel quick-bias"><span class="eyebrow">LECTURA DEL MERCADO · ${esc(h?.label??'Hoy')}</span><h2>${valid&&s.reading.available?esc(s.reading.reading):'Esperando datos'}</h2><p>Sesgo estructural de la sesión. La confirmación del movimiento y los motivos de espera aparecen debajo.</p></section><div class="quick-level-stack">${[['CALL WALL',s?.callWall?.strike,'call'],['IMÁN PRINCIPAL',s?.magnet,'magnet'],['PUT WALL',s?.putWall?.strike,'put']].map(([label,value,tone])=>`<section class="panel quick-level ${tone}"><span>${label}</span><strong>${number(valid?value as number|null:undefined)}</strong><small>${valid?'Referencia del snapshot de opciones':'Sin estructura vigente'}</small></section>`).join('')}</div><section class="panel quick-context"><span class="eyebrow">PRECIO DEL ACTIVO</span>${metric('Última operación · hasta 15 s',currentPrice)}<h3>${currentPrice==null?'Esperando precio reciente':'Precio reciente recibido'}</h3><p>Busca el recorrido desde el imán hacia el muro. Revisa debajo el objetivo, la invalidación y la confirmación de 2 minutos.</p></section></div>`;
}
