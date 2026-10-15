import type {Company} from './investment';
import {escapeHtml as esc} from './model';

export const investmentResearchPolicy = {
 version:'research-framework-v1',
 weights:[['Calidad y ventaja competitiva',20],['Solvencia y caja',15],['Crecimiento y resultados',15],['Valoración y margen',25],['Estructura y entrada',15],['Sector, macro y catalizadores',10]],
 pending:['Resultados trimestrales frente al consenso y guidance','Valoración histórica y competidores directos','Ventaja competitiva documentada','Estructura mensual y semanal','Sector, macro y catalizadores con fechas','Evaluación de opciones a largo plazo'],
} as const;
const valid=(v:number|null|undefined):v is number=>typeof v==='number'&&Number.isFinite(v);
const money=(v:number|null|undefined)=>valid(v)?new Intl.NumberFormat('en-US',{style:'currency',currency:'USD',maximumFractionDigits:2}).format(v):'Sin datos';
const pct=(v:number|null|undefined)=>valid(v)?`${v.toFixed(1)}%`:'Sin datos';
export function investmentThesis(c:Company,now=Date.now()) {
 const o=c.opportunity;
 const date=Date.parse(c.priceDate+'T00:00:00Z');
 const fresh=Number.isFinite(date)&&date<=now&&now-date<=7*86400000&&!!o&&Number.isFinite(o.at)&&o.at<=now&&now-o.at<=7*86400000;
 const metric=(id:string)=>c.metrics.find(m=>m.id===id)?.value;
 const price=fresh?metric('last_close'):null;
 const fair=fresh?o?.fairValue:null;
 const margin=fresh&&valid(price)&&price>0&&valid(fair)&&fair>0?(fair-price)/fair*100:null;
 const blocked=!c.eligible||c.coverage<85||c.status==='Riesgo alto'||c.status==='Especializado'||c.status==='Datos incompletos';
 const title=!fresh?'Revisar vigencia':blocked?'Completar investigación':!valid(margin)?'Valoración pendiente':margin>0?'Posible rebaja según el modelo':'Sin rebaja según el modelo';
 return {title,fresh,price,fair,margin,drop:fresh?metric('drawdown'):null,blocked};
}
export function investmentThesisPanel(c:Company,now=Date.now()):string {
 const t=investmentThesis(c,now),o=c.opportunity;
 const risks=[...(c.warnings??[]),...(c.sourceErrors??[]),...(o?.bear??[])].slice(0,3);
 const cells=[['Cierre · '+c.priceDate,money(t.price)],['Valor estimado',money(t.fair)],['Margen frente al valor',pct(t.margin)],['Caída desde máximo',pct(t.drop)]];
 return `<section class="ir-thesis" aria-label="Resumen de inversión"><header><span class="ir-eyebrow">LECTURA DE LARGO PLAZO</span><h3>${esc(t.title)}</h3><p>Una caída desde máximos no demuestra infravaloración. El valor es una estimación del modelo, no una garantía.</p></header><div class="ir-thesis-grid">${cells.map(([label,value])=>`<div><span>${esc(label)}</span><strong>${esc(value)}</strong></div>`).join('')}</div><div class="ir-thesis-columns"><article><h4>Zona y condiciones</h4><p>Soporte / invalidación: <b>${money(t.fresh?o?.invalidation:null)}</b></p><p>Resistencia de referencia: <b>${money(t.fresh?o?.target:null)}</b></p><p>Confirmación de entrada: pendiente. Los niveles diarios son referencias, no una señal de compra.</p></article><article><h4>Riesgos que revisar</h4>${risks.length?risks.map(r=>`<p>${esc(r)}</p>`).join(''):'<p>Sin alertas registradas; falta completar la investigación cualitativa.</p>'}<p>Cobertura fundamental ${pct(c.coverage)} · estado ${esc(c.status)}</p></article></div><details><summary>Marco de investigación · qué falta verificar</summary><p>El motor principal aplica estos pesos a la evidencia disponible. Los modelos anteriores quedan como análisis complementario.</p><div class="ir-thesis-grid">${investmentResearchPolicy.weights.map(([name,weight])=>`<div><span>${esc(name)}</span><b>${weight} puntos del marco</b></div>`).join('')}</div><ul>${investmentResearchPolicy.pending.map(p=>`<li>${esc(p)} · pendiente de verificar</li>`).join('')}</ul><p>Fuentes, fechas y cobertura son obligatorias. Datos ausentes no equivalen a cero. Ninguna puntuación representa una probabilidad de beneficio.</p></details></section>`;
}
