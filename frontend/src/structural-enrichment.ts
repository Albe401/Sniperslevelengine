import {scopeContractReport} from './contract-profile';
import {readMarketEvidence} from './market-evidence';
import {sharedLevelDetail} from './shared-level-detail';
import type {LiveState} from './live';
import type {StructuralCard} from './structural-map-preview';
import type {EtfReport,EtfSide} from './etf-dealers';
import {rankStrikes,enrichmentPanel,etfFlowPanel,type EtfFlow} from './etf-enrichment';
import {blockPressurePanel,defaultBlockPeriod} from './block-pressure';
import {cvdPanel} from './cvd';
import {escapeHtml as esc} from './model';
import './structural-enrichment.css';
const fmt=(n:number|null|undefined)=>n==null||!Number.isFinite(n)?'—':new Intl.NumberFormat('es',{notation:'compact',maximumFractionDigits:2}).format(n);
const sum=(ns:(number|null)[])=>ns.length&&ns.every(n=>n!=null&&Number.isFinite(n))?ns.reduce<number>((s,n)=>s+n!,0):null;
const selections=new Map<string,{expiry:string;strike?:number}>();
const views=new Map<string,EtfReport>();
/** Scope by actual horizon dates, not by a rolling calendar approximation. */
export const scopeStructuralReport=scopeContractReport;
export function structuralEvidenceReport(s:LiveState|undefined,symbol:string,active:boolean,card:StructuralCard,now:number):EtfReport|undefined {
 const snapshot=s?.optionSnapshotStatus,r=snapshot?.structuralEnrichment;
 if(!active||s?.symbol!==symbol||snapshot?.phase!=='ready'||!r?.enrichment||r.symbol!==symbol||r.at!==snapshot.observedMs||r.at!==card.at||!card.scopeReady||!card.expirations?.length||r.at>now||now-r.at>300000)return;
 const day=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});if(day.format(r.at)!==day.format(now))return;
 if(card.expirations.some(d=>!r.expirations.includes(d)))return;
 return scopeStructuralReport(r,card.expirations);
}
function evidenceBody(key:string,r:EtfReport):string {
 const selected=selections.get(key)??{expiry:''};
 if(selected.expiry&&!r.expirations.includes(selected.expiry))selected.expiry='';
 const view=selected.expiry?scopeStructuralReport(r,[selected.expiry]):r;
 const rankings=(['oi','volume','delta','gamma'] as const).map(metric=>({metric,label:{oi:'OI',volume:'Volumen',delta:'Delta absoluta',gamma:'Gamma / GEX'}[metric],rows:rankStrikes(view,metric)}));
 if(selected.strike!=null&&!view.rows.some(row=>row.strike===selected.strike))selected.strike=undefined;
 selections.set(key,selected);
 return `<label>Vencimiento <select data-structural-expiry><option value="">Todos de este horizonte</option>${r.expirations.map(d=>`<option value="${esc(d)}" ${selected.expiry===d?'selected':''}>${esc(d)}</option>`).join('')}</select></label><div class="structural-rankings">${rankings.map(group=>`<div><strong>${group.label} · CALL + PUT</strong>${group.rows.map(row=>`<button data-structural-strike="${row.strike}">${row.strike.toFixed(2)} · ${fmt(row.value)}</button>`).join('')||'<small>Sin cantidades completas</small>'}</div>`).join('')}</div><label>Strike <select data-structural-strike-select><option value="">Seleccionar nivel</option>${view.rows.map(row=>`<option value="${row.strike}" ${selected.strike===row.strike?'selected':''}>${row.strike.toFixed(2)}</option>`).join('')}</select></label>${enrichmentPanel(view,selected.strike,selected.expiry)}<p class="structural-evidence-method">GEX en USD por movimiento del 1%; delta en acciones equivalentes. Rankings de concentración, sin identificación de inventario dealer. Se incluyen todos los strikes de la cadena consultada, dentro de su banda de precios y vencimientos.</p>`;
}
export function structuralEnrichmentPanel(symbol:string,s:LiveState|undefined,active:boolean,now:number,cards:StructuralCard[],legend=""):string {
 const common=readMarketEvidence(s,symbol,active,now);
 const own=active&&s?.symbol===symbol&&s.publishedMs<=now&&now-s.publishedMs<=3000;
 const flow=s?.flow;
 const tracked=new Set(flow?.contract_windows?.map(c=>c.ticker)??[]).size;
 const data:EtfFlow|undefined=own&&flow?{symbol,at:s!.publishedMs,connected:s!.options?.phase==='streaming',tracked,subscribed:tracked,lastEventMs:s!.options?.lastEventMs??null,flow:{contract_windows:flow.contract_windows??[],windows:(flow.windows??[]).map(w=>({...w,buckets:w.buckets??{}}))}}:undefined;
 return `<details class="structural-enrichment" data-horizon-details="structural-enrichment" role="dialog" aria-label="Lectura detallada del mapa estructural"><summary>Cerrar lectura detallada</summary>${legend}<p>${esc(common.detail)}</p>${common.levels.map(l=>sharedLevelDetail(common,l.price)).join('')}<p>Contexto adicional del mapa. Los horizontes se solapan; sus cantidades no se suman como posiciones independientes. Los datos nuevos no alteran los porcentajes delta ni confirman defensa institucional.</p>${cards.map((card,i)=>{
 const r=structuralEvidenceReport(s,symbol,active,card,now),key=`${symbol}:${i}`;
 if(r)views.set(key,r);else views.delete(key);
 return `<details data-horizon-details="structural-evidence-${i}"><summary>${esc(card.name)} · detalle estructural</summary><div data-structural-evidence-host="${esc(key)}">${r?evidenceBody(key,r):'<p>Detalle pendiente: requiere cadena propia, reciente y coherente con este horizonte.</p>'}</div></details>`;
 }).join('')}<details data-horizon-details="structural-evidence-flow"><summary>CVD, flujo de opciones y bloques del activo</summary>${cvdPanel(s?.cvd,symbol,own,defaultBlockPeriod(now),now)}${etfFlowPanel(data,symbol,own,now)}${blockPressurePanel(s,symbol,own,now,undefined,false)}</details></details>`;
}
if(typeof document!=='undefined'){
 const update=(target:Element,strike?:number,expiry?:string)=>{
  const host=target.closest<HTMLElement>('[data-structural-evidence-host]'),key=host?.dataset.structuralEvidenceHost,r=key?views.get(key):undefined;
  if(!host||!key||!r)return;
  const selected=selections.get(key)??{expiry:''};if(expiry!=null){selected.expiry=expiry;selected.strike=undefined;}else selected.strike=strike;
  selections.set(key,selected);host.innerHTML=evidenceBody(key,r);
 };
 const open=(target:Element)=>{
  const card=target.closest<HTMLElement>('[data-structural-open]');
  const panel=card?.closest('.structural-map-preview')?.querySelector<HTMLDetailsElement>('.structural-enrichment');
  if(!panel||!card)return;panel.open=true;
  panel.querySelectorAll<HTMLDetailsElement>('[data-horizon-details^="structural-evidence-"]').forEach(d=>{d.open=d.dataset.horizonDetails===`structural-evidence-${card.dataset.structuralOpen}`;});
  panel.querySelector<HTMLElement>('summary')?.focus();
 };
 document.addEventListener('click',event=>{const target=event.target;if(target instanceof Element)open(target);});
 document.addEventListener('keydown',event=>{const target=event.target;if(!(target instanceof Element))return;
  if((event.key==='Enter'||event.key===' ')&&target.matches('[data-structural-open]')){event.preventDefault();open(target);}
  if(event.key==='Escape'){const panel=document.querySelector<HTMLDetailsElement>('.structural-enrichment[open]');if(panel){panel.open=false;panel.closest('.structural-map-preview')?.querySelector<HTMLElement>('[data-structural-open]')?.focus();}}
 });
 document.addEventListener('click',event=>{const button=(event.target as Element|null)?.closest<HTMLElement>('[data-structural-strike]');if(button)update(button,Number(button.dataset.structuralStrike));});
 document.addEventListener('change',event=>{const target=event.target;if(!(target instanceof HTMLSelectElement))return;if(target.matches('[data-structural-expiry]'))update(target,undefined,target.value);if(target.matches('[data-structural-strike-select]'))update(target,target.value?Number(target.value):undefined);});
}
