import type {LevelActivity,LevelActivityMetric} from './level-activity';
import {escapeHtml as esc} from './model';

type ActivityKind='acceptance'|'stock'|'option';
const number=new Intl.NumberFormat('es',{maximumFractionDigits:3});
const compact=new Intl.NumberFormat('en-US',{notation:'compact',maximumFractionDigits:0});
const sources:{kind:ActivityKind;key:'acceptance'|'stockSales'|'optionSales';label:string;short:string;unit:string;scope:string}[]=[
 {kind:'acceptance',key:'acceptance',label:'ACEPT.',short:'ACEP.',unit:'perfil',scope:'AMT · perfil observado · índice experimental de aceptación; no probabilidad'},
 {kind:'stock',key:'stockSales',label:'ACCIONES',short:'ACC.',unit:'acc.',scope:'Acciones · ventas agresivas inferidas · últimos 60 s · unidad: acciones'},
 {kind:'option',key:'optionSales',label:'OPCIONES',short:'OPC.',unit:'contr.',scope:'Opciones · ventas inferidas al BID · últimos 5 min · unidad: contratos'},
];
const finiteValue=(metric:LevelActivityMetric|undefined,kind:ActivityKind):number|null=>{
 const value=metric?.value;
 return typeof value==='number'&&Number.isFinite(value)&&value>=0&&(kind!=='acceptance'||value<=1)?value:null;
};
function intensity(metric:LevelActivityMetric|undefined,kind:ActivityKind):number {
 const value=finiteValue(metric,kind),raw=metric?.intensity;
 return value!=null&&value>0&&typeof raw==='number'&&Number.isFinite(raw)?Math.min(1,Math.max(0,raw)):0;
}

/** Complete style attribute for the INNER .day-map-level; all values are numeric. */
export function activityStyle(read:LevelActivity|undefined):string {
 const variables=sources.flatMap(source=>{
  const value=intensity(read?.[source.key],source.kind);
  // Separate source opacity, never the structural --node-color or score value.
  const fill=value>0?.035+.225*value:0,edge=value>0?.18+.72*value:0;
  return [`--activity-${source.kind}-fill:${fill.toFixed(4)}`,`--activity-${source.kind}-edge:${edge.toFixed(4)}`];
 });
 return `style="${variables.join(';')}"`;
}

function visibleValue(value:number|null,kind:ActivityKind,label:string):string {
 if(value==null)return '—';
 if(kind==='acceptance')return ['Alta','Media','Baja'].includes(label)?label:'—';
 if(value>0&&value<1)return '<1';
 return compact.format(value);
}

/** Independent source chips; a shared maximum does not become a trading signal. */
export function activityChips(read:LevelActivity|undefined):string {
 const captured=read?.captured===true;
 const chips=sources.map(source=>{
  const metric=read?.[source.key],value=finiteValue(metric,source.kind);
  const positive=value!=null&&value>0,highlight=positive&&intensity(metric,source.kind)>0;
  const maximum=highlight&&metric?.max===true,tied=maximum&&metric?.tied===true;
  const label=typeof metric?.label==='string'&&metric.label?metric.label:source.label;
  const detail=typeof metric?.detail==='string'&&metric.detail?metric.detail:'Sin cobertura verificable para esta fuente.';
  const amount=value==null?'Sin datos verificables':source.kind==='acceptance'?`${number.format(value)} de índice descriptivo (0–1), no calibrado`:`${number.format(value)} ${source.kind==='stock'?'acciones':'contratos'}`;
  const winner=maximum?`${source.kind==='acceptance'?'Mayor aceptación':'Mayor cantidad de ventas inferidas'} entre los niveles comparables de esta fuente${captured?' en la captura':''}.${tied?' Máximo compartido.':''}`:'';
  const title=[label,source.scope,captured?'Observación guardada en la captura; no lectura en vivo.':'',amount,detail,winner,'Color relativo dentro de su propia fuente; no compara acciones con contratos ni cambia la fuerza estructural.'].filter(Boolean).join(' · ');
  const classes=`level-activity-chip level-activity-${source.kind}${value==null?' is-pending':value===0?' is-zero':''}${maximum?' is-max':''}${tied?' is-tied':''}`;
  return `<span class="${classes}" role="img" title="${esc(title)}" aria-label="${esc(title)}"><span class="level-activity-label" aria-hidden="true"><span class="level-activity-label-long">${source.label}</span><span class="level-activity-label-short">${source.short}</span></span><span class="level-activity-quantity"><b class="level-activity-value">${esc(visibleValue(value,source.kind,label))}</b><span class="level-activity-unit">${source.unit}</span></span><span class="level-activity-max" aria-hidden="true">${maximum?'★':''}</span></span>`;
 }).join('');
 return `<div class="level-activity-chips${captured?' is-captured':''}" role="group" aria-label="Actividad separada por fuente${captured?' en la captura':''}">${chips}</div>`;
}
