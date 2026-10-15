import type {Company} from './investment';
import {escapeHtml as esc} from './model';

/** Evidence-first implementation of the supplied research framework.
 * Unconnected qualitative evidence reserves weight, never earns synthetic points.
 * Heuristic criteria are not calibrated probabilities or investment recommendations. */
export function analyzePrimaryInvestment(c:Company,now=Date.now()) {
 const g=(id:string)=>{const v=c.metrics.find(m=>m.id===id)?.value;return typeof v==='number'&&Number.isFinite(v)?v:null;};
 const freshDate=(s:string,days:number)=>{const t=Date.parse(s+'T00:00:00Z');return Number.isFinite(t)&&t<=now&&now-t<=days*86400000;};
 const financialFresh=freshDate(c.period,200)&&freshDate(c.asOf,7)&&c.eligible;
 const priceFresh=freshDate(c.priceDate,7);
 const o=c.opportunity;
 const modelFresh=priceFresh&&!!o&&Number.isFinite(o.at)&&o.at<=now&&now-o.at<=7*86400000;
 type Factor={label:string;weight:number;value:number|null;source:string;points:number|null};
 const factor=(label:string,weight:number,id:string,good:(v:number)=>boolean,allowed=financialFresh):Factor=>{const value=allowed?g(id):null;return {label,weight,value,source:c.metrics.find(m=>m.id===id)?.source??'Fuente pendiente',points:value==null?null:good(value)?weight:0};};
 const missing=(label:string,weight:number):Factor=>({label,weight,value:null,source:'Evidencia pendiente de conectar o verificar',points:null});
 const price=modelFresh?g('last_close'):null,fair=modelFresh?o?.fairValue:null;
 const margin=price!=null&&price>0&&fair!=null&&Number.isFinite(fair)&&fair>0?(fair-price)/fair*100:null;
 const modelFactor:Factor={label:'Margen del modelo aproximado > 0%',weight:10,value:margin,source:'Modelo de escenarios existente; no DCF normalizado',points:margin==null?null:margin>0?10:0};
 const definitions:[string,Factor[]][]=[
  ['Calidad y ventaja competitiva',[factor('ROIC ≥ 10%',5,'roic',v=>v>=10),factor('Margen operativo positivo',5,'operating_margin',v=>v>0),missing('Ventaja competitiva documentada',10)]],
  ['Solvencia y caja',[factor('FCF positivo',5,'fcf_computed',v=>v>0),factor('Deuda neta / EBITDA ≤ 3×',5,'net_debt_ebitda',v=>v<=3),factor('Cobertura de intereses ≥ 2×',5,'interest_cover',v=>v>=2)]],
  ['Crecimiento y resultados',[factor('Ingresos interanuales crecientes',5,'revenue_yoy',v=>v>0),factor('BPA interanual creciente',5,'eps_yoy',v=>v>0),missing('Trimestres, consenso, guidance y revisiones',5)]],
  ['Valoración y margen',[modelFactor,missing('Comparación histórica de valoración',5),missing('Competidores directos y múltiplos',5),missing('Valor normalizado y sensibilidad',5)]],
  ['Estructura y entrada',[factor('EMA200 diaria disponible',5,'ema200',v=>v>0,priceFresh),missing('Estructura semanal y mensual',5),missing('Confirmación de precio y volumen',5)]],
  ['Sector, macro y catalizadores',[missing('Fuerza relativa y contexto macro',5),missing('Catalizadores verificados y fechas',5)]],
 ];
 const blocks=definitions.map(([name,factors])=>({name,factors,weight:factors.reduce((s,f)=>s+f.weight,0),available:factors.reduce((s,f)=>s+(f.points==null?0:f.weight),0),earned:factors.reduce((s,f)=>s+(f.points??0),0)}));
 const available=blocks.reduce((s,b)=>s+b.available,0),earned=blocks.reduce((s,b)=>s+b.earned,0);
 const critical=c.status==='Riesgo alto'||(financialFresh&&(g('net_debt_ebitda')??0)>5)||['equity','fcf_computed','net_income_loss_attributable_common_shareholders'].some(id=>{const v=financialFresh?g(id):null;return v!=null&&v<=0;});
 const status=c.status==='Especializado'?'Requiere modelo sectorial':critical?'Riesgo prioritario':!financialFresh||!priceFresh?'Revisar datos y vigencia':available<85?'Investigación incompleta':margin!=null&&margin>0?'Candidata a estudiar':'En vigilancia';
 return {version:'primary-research-v1',blocks,available,earned,score:available?earned:null,status,margin,critical,
  pending:blocks.flatMap(b=>b.factors.filter(f=>f.points==null).map(f=>f.label))};
}
export function primaryInvestmentPanel(c:Company,now=Date.now()):string {
 const r=analyzePrimaryInvestment(c,now);
 return `<section class="ir-thesis ir-primary" aria-label="Evaluación principal de compañía"><header><span class="ir-eyebrow">MOTOR PRINCIPAL · INVESTIGACIÓN INTEGRAL</span><h3>${esc(r.status)}</h3><p>${r.score==null?'Sin evidencia puntuable':`${r.earned.toFixed(0)} puntos sustentados de 100`} · cobertura del marco ${r.available}%</p><p>Los criterios pendientes reservan su peso. No se renormaliza una investigación parcial a 100 ni se presenta como probabilidad.</p></header><div class="ir-thesis-grid">${r.blocks.map(b=>`<article><h4>${esc(b.name)}</h4><strong>${b.earned}/${b.weight}</strong><p>Evidencia disponible: ${b.available}/${b.weight}</p><details><summary>Ver evidencias y fuentes</summary>${b.factors.map(f=>`<p><b>${esc(f.label)}</b>: ${f.points==null?'Pendiente':`${f.points}/${f.weight} puntos · valor ${f.value?.toFixed(2)}`}<br><small>${esc(f.source)}</small></p>`).join('')}</details></article>`).join('')}</div><p>Umbrales iniciales de investigación, pendientes de validación por sector. La EMA disponible no demuestra tendencia ni confirma una entrada.</p><details><summary>Información necesaria para completar la tesis (${r.pending.length})</summary><ul>${r.pending.map(p=>`<li>${esc(p)}</li>`).join('')}</ul></details></section>`;
}
