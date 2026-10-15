import {invoke,isTauri} from './snapshot-observation';
import {escapeHtml as esc} from './model';
export interface ComponentRead {version:string;symbol:string;at:number;phase:string;available:boolean;call:number|null;put:number|null;coverage:number;optionCoverage:number;selectedWeight:number;priceCoverage:number;leaders:{symbol:string;contribution:number}[];dailyLeaders:{symbol:string;contribution:number}[];openingLeaders:{symbol:string;contribution:number}[];missing:string[];relation:string;asOf:string;reason:string;quality:string}
const pending=():ComponentRead=>({version:'qqq-components-v2',symbol:'QQQ',at:0,phase:'Pendiente',available:false,call:null,put:null,coverage:0,optionCoverage:0,selectedWeight:0,priceCoverage:0,leaders:[],dailyLeaders:[],openingLeaders:[],missing:[],relation:'Pendiente',asOf:'',reason:'Esperando evidencia del motor compartido',quality:'Pendiente'});
let cached:ComponentRead|undefined,busy=false,wasEnabled=false,retryAt=0;
let period:'leaders'|'dailyLeaders'|'openingLeaders'|undefined;
export function usableComponentReport(r:ComponentRead|undefined,now=Date.now()):r is ComponentRead{
 if(!r||r.version!=='qqq-components-v2'||r.symbol!=='QQQ'||!Number.isFinite(r.at)||r.at>now||now-r.at>5000||![r.coverage,r.optionCoverage,r.selectedWeight,r.priceCoverage].every(n=>Number.isFinite(n)&&n>=0&&n<=100))return false;
 if(r.coverage>r.selectedWeight||r.optionCoverage>r.coverage||r.priceCoverage>r.selectedWeight||(r.call==null)!==(r.put==null)||r.available!==(r.call!=null&&r.put!=null))return false;
 if(r.available&&(!['Premarket','Sesión RTH'].includes(r.phase)||r.phase==='Sesión RTH'&&(r.optionCoverage<40||r.optionCoverage<r.selectedWeight*.8)))return false;
 if(r.call!=null&&(!Number.isFinite(r.call)||r.call<0||r.call>100||r.put!==100-r.call||!r.available||r.coverage<40||r.coverage<r.selectedWeight*.8))return false;
 return [r.leaders,r.dailyLeaders,r.openingLeaders].every(rows=>Array.isArray(rows)&&rows.length<=12&&new Set(rows.map(v=>v.symbol)).size===rows.length&&rows.every(v=>/^[A-Z.]+$/.test(v.symbol)&&Number.isFinite(v.contribution)));
}
export function currentComponentRead(now=Date.now()){return usableComponentReport(cached,now)?cached:pending();}
export function componentLabel(r:ComponentRead){return `Componentes QQQ · ${r.phase==='Premarket'?'PRE · ':''}${r.call==null?'Pendiente':`CALL ${r.call}% / PUT ${r.put}%`} · ${r.relation}`;}
export function componentQuickPercent(r=currentComponentRead()):string {return `<span class="qqq-percent-up">${r.call==null?'—':r.call+'%'}</span><span class="qqq-percent-down">${r.put==null?'—':r.put+'%'}</span>`;}
export function componentQuickActors(r=currentComponentRead()):string {
 const pre=r.phase==='Premarket',actors=pre?r.dailyLeaders:r.leaders,horizon=pre?'PRE · desde cierre anterior':'últimos 5 minutos';
 const rows=(positive:boolean)=>actors.filter(l=>positive?l.contribution>0:l.contribution<0).sort((a,b)=>Math.abs(b.contribution)-Math.abs(a.contribution));
 return [true,false].map(positive=>{const list=rows(positive),names=list.map(l=>l.symbol);return `<span class="qqq-quick-${positive?'up':'down'}" title="${esc(names.join(', ')||'Sin evidencia suficiente')} · Aporte estimado al QQQ ${horizon}"><b>${positive?'Apoyan ↑':'Presionan ↓'} · ${pre?'PRE':'5m'}</b><span>${esc(names.slice(0,4).join(' · ')||'Pendiente')}${names.length>4?' +'+(names.length-4):''}</span></span>`;}).join('');
}
export function componentCard(r=currentComponentRead()):string {
 const selectedPeriod=period??(r.phase==='Premarket'?'dailyLeaders':'leaders');
 const selected=r[selectedPeriod]??[],up=selected.filter(l=>l.contribution>0),down=selected.filter(l=>l.contribution<0);
 const periodLabel=selectedPeriod==='leaders'?'5 min':selectedPeriod==='dailyLeaders'?'desde cierre anterior':'desde apertura RTH';
 const list=(rows:typeof up)=>rows.map(l=>`<span class="qqq-component-symbol" title="Aporte estimado ${periodLabel}: ${l.contribution>=0?'+':''}${l.contribution.toFixed(3)} puntos porcentuales">${esc(l.symbol)} <b>${l.contribution>=0?'+':''}${l.contribution.toFixed(3)}</b></span>`).join('')||'<small>Sin evidencia suficiente</small>';
 return `<aside class="qqq-component-card" aria-label="Componentes QQQ"><strong>${esc(componentLabel(r))}</strong><div class="qqq-component-assets"><div class="qqq-component-up"><small>Apoyando ↑ · ${periodLabel}</small><div>${list(up)}</div></div><div class="qqq-component-down"><small>Presionando ↓ · ${periodLabel}</small><div>${list(down)}</div></div></div><small>Lectura ${r.coverage.toFixed(1)}% · Opciones ${r.optionCoverage.toFixed(1)}% · Precio ${r.priceCoverage.toFixed(1)}% de QQQ</small><div class="qqq-period-tools">${[['leaders','5 min'],['dailyLeaders','Día'],['openingLeaders','Apertura']].map(([key,label])=>`<button data-qqq-period="${key}" aria-pressed="${selectedPeriod===key}">${label}</button>`).join('')}<small>Invesco ${esc(r.asOf||'pendiente')}</small></div><details><summary>Método</summary><p>${esc(r.reason)} Faltantes: ${esc(r.missing.join(', ')||'ninguno')}. Aportes estimados ${periodLabel}, en puntos porcentuales del ETF. No representan beneficios de opciones.</p></details></aside>`;
}
const rendered=new WeakMap<HTMLElement,string>();
function stableMarkup(el:HTMLElement,html:string){if(rendered.get(el)!==html){el.innerHTML=html;rendered.set(el,html);}}
function paint(){
 if(typeof document==='undefined')return;const r=currentComponentRead();
 document.querySelectorAll<HTMLElement>('[data-qqq-components]').forEach(el=>{const open=el.querySelector('details')?.open??false;stableMarkup(el,componentCard(r));const details=el.querySelector('details');if(details)details.open=open;el.querySelectorAll<HTMLButtonElement>('[data-qqq-period]').forEach(b=>b.onclick=()=>{period=b.dataset.qqqPeriod as typeof period;paint();});});
 document.querySelectorAll<HTMLElement>('[data-qqq-component-line]').forEach(el=>{if(el.closest('.qqq-direction-components'))stableMarkup(el,componentQuickPercent(r));else if(el.textContent!==componentLabel(r))el.textContent=componentLabel(r);el.title=r.reason;});
 document.querySelectorAll<HTMLElement>('[data-qqq-component-actors]').forEach(el=>stableMarkup(el,componentQuickActors(r)));
}
async function poll(){
 if(typeof document==='undefined'||busy||!isTauri())return;
 const needed=!!document.querySelector('[data-qqq-components],[data-qqq-component-line]');
 if(!needed&&!wasEnabled||needed&&Date.now()<retryAt)return;
 busy=true;
 try{const result=await invoke<{connected:boolean;report?:ComponentRead}>('get_qqq_component_inputs',{enabled:needed});wasEnabled=needed;cached=!result.connected?undefined:usableComponentReport(result.report)?result.report:usableComponentReport(cached)?cached:undefined;retryAt=0;}
 catch{if(!usableComponentReport(cached))cached=undefined;retryAt=Date.now()+2000;}
 finally{busy=false;paint();}
}
setInterval(()=>{paint();void poll();},2000);
