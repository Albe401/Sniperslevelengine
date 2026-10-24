import type {LiveState} from './live';
import {escapeHtml as esc} from './model';
import './massive-exposure-panel.css';
export interface MassiveExposureRow {strike:number;netGex:number;callGex?:number;putGex?:number}
export interface MassiveExposureView {symbol:string;mode:string;expiry:string;at:number;complete:boolean;historical:boolean;gammaUsed:number;eligible:number;net:number;rows:MassiveExposureRow[];reason:string}
const dateFormat=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'}),clockFormat=new Intl.DateTimeFormat('es-ES',{timeZone:'America/New_York',day:'2-digit',month:'2-digit',hour:'2-digit',minute:'2-digit'});
const compact=new Intl.NumberFormat('es-ES',{notation:'compact',maximumFractionDigits:2}),full=new Intl.NumberFormat('es-ES',{maximumFractionDigits:2});
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v),clock=(v:unknown):v is number=>finite(v)&&Number.isSafeInteger(v)&&v>0&&v<=8640000000000000;
/** Read already-calculated Massive OI exposure. No Gexbot state, new Greeks, or provider request. */
export function readMassiveExposure(s:LiveState|undefined,symbol:string,active:boolean,now:number):MassiveExposureView {
 const empty:MassiveExposureView={symbol,mode:'',expiry:'',at:0,complete:false,historical:true,gammaUsed:0,eligible:0,net:0,rows:[],reason:'Sin captura de exposición Massive del activo'};
 const fail=(reason:string)=>({...empty,reason});
 if(!clock(now)||s?.symbol!==symbol||!clock(s.publishedMs)||s.publishedMs>now)return empty;
 const snapshot=s.optionSnapshotStatus;if(snapshot?.phase!=='ready')return fail('Esperando una captura de opciones Massive');
 const horizons=Array.isArray(snapshot.horizons)?snapshot.horizons:[],h=horizons.find(h=>h?.mode==='DAY')??horizons.find(h=>h?.mode==='NEXT_SESSION');
 if(!h||horizons.filter(x=>x?.mode===h.mode).length!==1||!Array.isArray(h.expirations)||h.expirations.length!==1)return fail('Sin vencimiento individual verificable; no se suman horizontes');
 if(!clock(h.observedMs)||h.observedMs>s.publishedMs)return fail('Reloj de exposición incompatible');
 if(snapshot.observedMs!=null&&snapshot.observedMs!==h.observedMs)return fail('La exposición y la captura no comparten la misma hora');
 const expiry=h.expirations[0],sourceDay=dateFormat.format(h.observedMs);
 if(!/^\d{4}-\d{2}-\d{2}$/.test(expiry)||(h.mode==='DAY'?expiry!==sourceDay:expiry<=sourceDay))return fail('Vencimiento incompatible con la captura');
 const e=h.exposure;
 if(!e||!Number.isSafeInteger(e.gammaUsed)||!Number.isSafeInteger(e.eligible)||e.gammaUsed<=0||e.eligible<e.gammaUsed||!finite(e.netGex)||!Array.isArray(e.strikes)||!e.strikes.length)return fail('Sin gamma/OI verificables para mostrar exposición');
 if(e.strikes.length>20000)return fail('Perfil demasiado grande; no se recortan strikes silenciosamente');
 const rows=e.strikes as MassiveExposureRow[],strikes=new Set<number>();let sum=0,correction=0,absolute=0;
 for(const row of rows){
  if(!row||!finite(row.strike)||row.strike<=0||!finite(row.netGex)||strikes.has(row.strike))return fail('Perfil ambiguo: strikes duplicados o valores inválidos');strikes.add(row.strike);
  const {callGex:call,putGex:put}=row;
  if(call!==undefined&&(!finite(call)||call<0)||put!==undefined&&(!finite(put)||put<0))return fail('Componentes CALL/PUT de gamma inválidos');
  if(call!==undefined&&put!==undefined&&Math.abs(call-put-row.netGex)>Math.max(.01,16*Number.EPSILON*Math.max(call,put)))return fail('Neto por strike incompatible con CALL menos PUT');
  const value=row.netGex-correction,next=sum+value;correction=(next-sum)-value;sum=next;absolute+=Math.abs(row.netGex);
 }
 if(!finite(sum)||!finite(absolute)||Math.abs(sum-e.netGex)>Math.max(.01,64*Number.EPSILON*rows.length*absolute))return fail('El subtotal no coincide con los strikes de esta captura');
 return {symbol,mode:h.mode,expiry,at:h.observedMs,complete:e.gexComplete===true&&e.gammaUsed===e.eligible,historical:!active||now-s.publishedMs>15000||now-h.observedMs>300000||dateFormat.format(now)!==sourceDay,gammaUsed:e.gammaUsed,eligible:e.eligible,net:e.netGex,rows:[...rows].sort((a,b)=>a.strike-b.strike),reason:''};
}
const pages=new Map<string,number>();export const MASSIVE_EXPOSURE_PAGE=100;
export function massiveExposurePanel(s:LiveState|undefined,symbol:string,active:boolean,now:number,standalone=false):string {
 const rootClass=standalone?'structural-map-preview massive-exposure-panel massive-exposure-standalone':'massive-exposure-panel';
 const view=readMassiveExposure(s,symbol,active,now),title='<strong>EXPOSICIÓN GEX · MASSIVE</strong>';
 if(view.reason)return `<section class="${rootClass}" data-massive-exposure aria-label="Exposición gamma propia por strike"><header>${title}</header><p class="massive-exposure-empty">${esc(view.reason)}. Sin datos no equivale a exposición cero.</p></section>`;
 const key=JSON.stringify([symbol,view.mode,view.expiry,dateFormat.format(view.at)]),count=Math.ceil(view.rows.length/MASSIVE_EXPOSURE_PAGE),page=Math.max(0,Math.min(count-1,pages.get(key)??0)),start=page*MASSIVE_EXPOSURE_PAGE,visible=view.rows.slice(start,start+MASSIVE_EXPOSURE_PAGE),peak=view.rows.reduce((n,r)=>Math.max(n,Math.abs(r.netGex)),0);
 const positive=view.rows.filter(r=>r.netGex>0).length,negative=view.rows.filter(r=>r.netGex<0).length,zero=view.rows.length-positive-negative;
 const value=(n:number|undefined)=>n==null?'—':compact.format(n),cell=(n:number|undefined,side='')=>`<td class="${side}" title="${n==null?'No suministrado':esc(full.format(n)+' USD por 1%')}">${value(n)}</td>`;
 return `<section class="${rootClass}" data-massive-exposure data-exposure-key="${esc(key)}" data-exposure-page-index="${page}" aria-label="Exposición gamma propia por strike"><header>${title}<span>${view.complete?'CADENA RECIBIDA COMPLETA':'SUBTOTAL PARCIAL'}${view.historical?' · HISTÓRICO':''}</span></header><p class="massive-exposure-source">${esc(symbol)} · ${view.mode==='DAY'?'HOY / DAY':'PRÓXIMA SESIÓN'} · ${esc(view.expiry)} · captura ${esc(clockFormat.format(view.at))} ET · ${view.gammaUsed}/${view.eligible} contratos con gamma.</p><div class="massive-exposure-summary"><span>Neto observado <b>${value(view.net)} USD / 1%</b></span><span>${view.rows.length} strikes · + ${positive} / − ${negative} / 0 ${zero}</span></div><div class="massive-exposure-pages"><button data-massive-exposure-page="${page-1}" ${page===0?'disabled':''}>Anterior</button><span>${start+1}–${Math.min(view.rows.length,start+MASSIVE_EXPOSURE_PAGE)} de ${view.rows.length} · ${page+1}/${count}</span><button data-massive-exposure-page="${page+1}" ${page===count-1?'disabled':''}>Siguiente</button></div><div class="massive-exposure-table" tabindex="0" aria-label="Todos los strikes recibidos de este vencimiento"><table><thead><tr><th>STRIKE</th><th>GEX CALL</th><th>GEX PUT</th><th>GEX OI NETO</th><th>PERFIL NETO</th></tr></thead><tbody>${visible.map(row=>{const positive=row.netGex>0,width=peak?50*Math.abs(row.netGex)/peak:0;return `<tr data-massive-exposure-strike="${row.strike}" data-massive-gex="${row.netGex}"><th>${full.format(row.strike)}</th>${cell(row.callGex,'massive-gex-call')}${cell(row.putGex,'massive-gex-put')}${cell(row.netGex,positive?'massive-gex-call':row.netGex<0?'massive-gex-put':'')}<td><div class="massive-exposure-bar"><i class="${positive?'positive':'negative'}" style="left:${positive?50:50-width}%;width:${width}%"></i></div></td></tr>`;}).join('')}</tbody></table></div><small class="massive-exposure-method">Gamma × OI × multiplicador × precio² × 0,01; neto CALL − PUT. USD por movimiento del 1% · escala de barras común a todos los strikes recibidos. ${view.complete?'':'Cadena incompleta: el signo del total no está confirmado. '}No es volumen ejecutado ni inventario dealer.</small></section>`;
}
export function bindMassiveExposurePanel(refresh:()=>void){
 document.querySelectorAll<HTMLButtonElement>('[data-massive-exposure-page]').forEach(button=>button.onclick=()=>{const key=button.closest<HTMLElement>('[data-exposure-key]')?.dataset.exposureKey,page=Number(button.dataset.massiveExposurePage);if(!button.isConnected||button.disabled||!key||!Number.isSafeInteger(page)||page<0)return;pages.set(key,page);if(pages.size>32)pages.delete(pages.keys().next().value!);refresh();});
}
