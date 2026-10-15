import {escapeHtml as esc} from './model';
export interface MapLayer {name:string;kind:'observed'|'derived'|'interpretation';asOf:number|null;available:boolean;source:string;method:string}
/** Descriptive timestamps only; no invented universal coherence threshold. */
export function mapDataQuality(layers:MapLayer[]){
 const known=layers.filter(l=>l.available&&l.asOf!=null&&Number.isFinite(l.asOf)&&l.asOf>0);
 const complete=layers.every(l=>l.available)&&known.length===layers.length;
 const skewMs=known.length>=2?Math.max(...known.map(l=>l.asOf!))-Math.min(...known.map(l=>l.asOf!)):null;
 const status=!complete?'PARCIAL · horas o cobertura pendientes':skewMs===0?'AS-OF COINCIDENTE':'MIXED-AS-OF';
 const html=`<details class="map-data-quality"><summary>Calidad del mapa: ${status}${skewMs==null?'':` · separación ${skewMs} ms`}</summary><p>Snapshot de OI, ventanas de flujo y velas cerradas tienen frecuencias diferentes. La separación temporal no prueba incoherencia ni coherencia causal.</p>${layers.map(l=>`<p><b>${esc(l.name)}</b> · ${l.kind==='observed'?'Observado':l.kind==='derived'?'Derivado':'Interpretación'} · ${l.available?'Disponible':'Pendiente'} · ${l.asOf!=null&&Number.isFinite(l.asOf)&&l.asOf>0?esc(new Date(l.asOf).toISOString()):'Hora no aportada'} · ${esc(l.source)} · ${esc(l.method)}</p>`).join('')}</details>`;
 return {status,skewMs,complete,html};
}
