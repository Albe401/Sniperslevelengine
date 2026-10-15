import {analyzePrimaryInvestment} from './investment-primary-engine';
import type {Company} from './investment';
import {investmentThesis} from './investment-thesis';
import {escapeHtml} from './model';
import './investment-shortlist.css';

const esc=(v:unknown)=>escapeHtml(String(v??''));
export function shortlistTable(companies:Company[],saved:Record<string,number>,compare:string[]):string {
 return '<div class="ir-company-grid">'+companies.map(c=>{
 const r=analyzePrimaryInvestment(c),t=investmentThesis(c);
 const price=c.metrics.find(m=>m.id==='last_close')?.value;
 const money=(v:number|null|undefined)=>v!=null&&Number.isFinite(v)?'$'+v.toFixed(2):'Sin datos';
 return `<article class="ir-company-card" data-ir-ticker="${esc(c.ticker)}"><header><button class="ir-shortlist-symbol" data-ir-open="${esc(c.ticker)}">${esc(c.ticker)}</button><span class="ir-badge">${esc(r.status)}</span></header><h3>${esc(c.name)}</h3><p>${esc(c.industry)}</p><div class="ir-card-numbers"><div><small>Cierre · ${esc(c.priceDate)}</small><strong>${money(t.fresh?price:null)}</strong></div><div><small>Valor estimado</small><strong>${money(t.fair)}</strong></div></div><p class="ir-card-margin">Margen del modelo: ${t.margin==null?'Sin datos':t.margin.toFixed(1)+'%'}</p><p>${r.score==null?'Sin evidencia puntuable':r.earned+' puntos sustentados / 100'} · cobertura ${r.available}%</p><footer><button data-ir-open="${esc(c.ticker)}">Abrir análisis completo ↗</button><button data-ir-save="${esc(c.ticker)}" aria-pressed="${!!saved[c.ticker]}">${saved[c.ticker]?'★ Guardada':'☆ Guardar'}</button><label><input type="checkbox" data-ir-compare="${esc(c.ticker)}" ${compare.includes(c.ticker)?'checked':''}> Comparar</label></footer></article>`;
 }).join('')+'</div>';
}

export function radarWorkspace():string {
 return `<section class="ir-workspace"><div><span class="ir-eyebrow">INVESTIGACIÓN DE COMPAÑÍAS</span><h2>Radar nuevo de compañías</h2><p>Explora los cuadros de compañías. Abre una para ver el análisis integral del nuevo marco.</p></div><div class="ir-horizons" role="group" aria-label="Horizonte de investigación"><button data-ir-horizon="long" aria-pressed="true">Largo plazo <small>Fundamentos y tesis</small></button><button data-ir-horizon="day" aria-pressed="false">Intradía <small>Noticias y catalizadores</small></button></div><div class="ir-news-design"><span class="ir-news-pending">MOTOR DE NOTICIAS · PENDIENTE</span><p>Próxima lectura: positiva, negativa, mixta o sin datos, con motivos, fecha y fuentes.</p></div></section>`;
}
