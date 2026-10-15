export interface TopFiveVolumeMetrics {
 notionalUsd:number|null;change10Pct:number|null;change30Pct:number|null;rvolPremarket:number|null;rvolSessions:number;
}
const money=new Intl.NumberFormat('es',{style:'currency',currency:'USD',notation:'compact',maximumFractionDigits:2});
const valid=(value:number|null|undefined):value is number=>value!=null&&Number.isFinite(value);
const change=(value:number|null|undefined)=>valid(value)?`${value>0?'+':''}${value.toFixed(1)}%`:'Sin datos';
export function topFiveVolumeMetrics(metrics:TopFiveVolumeMetrics|undefined):string {
 const samples=metrics?.rvolSessions??0;
 const rvol=valid(metrics?.rvolPremarket)&&metrics!.rvolPremarket!>=0&&samples>=3?`${metrics!.rvolPremarket!.toFixed(2)}×`:'Sin datos';
 return `<dl class="top-five-volume-context"><div><dt>Monto negociado aprox.</dt><dd>${valid(metrics?.notionalUsd)&&metrics!.notionalUsd!>=0?money.format(metrics!.notionalUsd!):'Sin datos'}</dd></div><div><dt>Volumen 10m vs. 10m anteriores</dt><dd>${change(metrics?.change10Pct)}</dd></div><div><dt>Volumen 30m vs. 30m anteriores</dt><dd>${change(metrics?.change30Pct)}</dd></div><div><dt>RVOL premarket</dt><dd>${rvol}</dd></div></dl><small class="top-five-volume-method">RVOL: mismo tramo desde 04:00 ET, hasta 09:30; ${samples}/5 sesiones previas con datos, mínimo 3. Cambios: volumen observado al corte; períodos sin datos no se rellenan.</small>`;
}
