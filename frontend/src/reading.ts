import { escapeHtml as esc, price } from './model';
import { rangeFresh, type RangeSnapshot } from './daily-range';
export interface StructuralReading {
  available:boolean;side:string;reading:string;strength:string;reason:string;
  bias:number|null;callScore:number|null;putScore:number|null;
}
export interface Alignment {status:string;side:string;callCount:number;putCount:number;neutralCount:number;unavailableCount:number}
interface Snapshot extends RangeSnapshot {
  horizons?:{mode:string;label:string;structure:{reading?:StructuralReading}}[];
  structuralAlignment?:Alignment|null;
}
export function structuralReadingPanel(snapshot:Snapshot|undefined,connected:boolean,now=Date.now()):string {
  const fresh=connected&&snapshot?.phase==='ready'&&rangeFresh(snapshot,now);
  const a=snapshot?.structuralAlignment;
  const labels:Record<string,string>={'FULL ALIGNMENT':'Alineación completa','PARTIAL ALIGNMENT':'Alineación parcial',MIXED:'Horizontes en conflicto',NEUTRAL:'Neutral','INSUFFICIENT DATA':'Datos insuficientes','NEUTRAL / MIXED':'Neutral / mixto'};
  return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">LECTURA ESTRUCTURAL · SIN OVERLAYS</span><h2>Alineación entre horizontes</h2></div><span class="tag">${fresh&&a?esc(labels[a.status]??a.status):'Sin lectura vigente'}</span></div>
  <div class="table-wrap"><table><thead><tr><th>HORIZONTE</th><th>LECTURA</th><th>PUNTUACIÓN CALL / PUT</th><th>SESGO</th></tr></thead><tbody>${(snapshot?.horizons??[]).map(h=>{
    const r=h.structure.reading;const usable=fresh&&r?.available;
    return `<tr><td>${esc(h.label)}</td><td>${usable?esc(r.reading):'Esperando datos'}<small class="exposure-dates">${esc(r?.reason??'Sin base estructural')}</small></td><td>${usable&&r.callScore!=null&&r.putScore!=null?`${price(r.callScore)} / ${price(r.putScore)}`:'—'}</td><td>${usable&&r.bias!=null?price(r.bias):'—'}</td></tr>`;
  }).join('')||'<tr><td colspan="4">Esperando los tres horizontes de opciones.</td></tr>'}</tbody></table></div>
  <div class="panel-caption">${fresh&&a?`CALL ${a.callCount} · PUT ${a.putCount} · neutrales ${a.neutralCount} · no disponibles ${a.unavailableCount}. `:''}La alineación completa exige los tres horizontes en la misma dirección. Pesos heredados: OI 40 %, volumen acumulado 25 %, dominancia 20 % y balance GEX 15 %. Puntuaciones del modelo, no probabilidades. Esta vista muestra la base estructural; los ajustes de flujo y TRF aparecen en Lectura integrada. No autoriza entradas.</div></section>`;
}
