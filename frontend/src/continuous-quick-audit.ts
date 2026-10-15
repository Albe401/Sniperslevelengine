import {vannaEngine} from './vanna-engine';
import {intradayAtr} from './intraday-atr';
import type {LiveState} from './live';
import {buildIntradayControl} from './intraday-control';
import {quickReadingPanels} from './quick-reading-panels';
import {readDirectionScore} from './direction-scanner';
import {structuralCards,structuralAlignment} from './structural-map-preview';
import {escapeHtml as esc} from './model';
export interface QuickAudit {vannaEngine?:import('./vanna-engine').VannaEngine;vanna?:import('./vanna-confluence').VannaRead;at:number;symbol:string;status:string;issues:string[];detail:string;ms:number}
const key='snipers-quick-audit-v1';
const date=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
export function marketOpen(s:LiveState|undefined,now:number):boolean {
 const session=s?.chartHistory?.sessions.find(x=>x.date===date.format(now));
 return !!session&&Number.isFinite(session.openMs)&&Number.isFinite(session.closeMs)&&session.openMs<=now&&now<session.closeMs;
}
export function auditQuick(s:LiveState|undefined,symbol:string,active:boolean,now:number):QuickAudit {
 const start=performance.now(),issues:string[]=[];
 try {
 const r=buildIntradayControl(s,symbol,active,now),d=readDirectionScore(s,r,active,now);
 const cards=structuralCards(s,symbol,active,now),alignment=structuralAlignment(cards);
 const html=quickReadingPanels(s,symbol,active,r,now);
 if(!active||s?.symbol!==symbol||!Number.isFinite(s?.publishedMs)||s!.publishedMs>now||now-s!.publishedMs>3000)issues.push('Publicación desconectada o caducada');
 if(!r.priceNow?.ready)issues.push('Precio comparable pendiente');
 if(d.call==null)issues.push('Confluencia CALL/PUT pendiente');
 if(d.call!=null&&d.put!=null&&(!Number.isFinite(d.call+d.put)||Math.abs(d.call+d.put-100)>.01))issues.push('ERROR: porcentajes CALL/PUT inválidos');
 const sign=d.side==='CALL'?1:d.side==='PUT'?-1:0;
 if(sign&&r.priceNow?.ready&&r.priceNow.direction!=null&&sign*r.priceNow.direction<0)issues.push('Divergencia entre confluencia y precio');
 if(d.cvd?.status==='pending')issues.push('CVD sin confirmación suficiente');
 const atr=intradayAtr(s,symbol,active,now);
 if(atr.value==null)issues.push('ATR intradía pendiente');
 if(!atr.daily)issues.push('ATR diario pendiente');
 if(r.expectedMove==null)issues.push('Rango esperado diario pendiente');
 if(r.flowControl?.quality!=='ready')issues.push('Flujo de opciones pendiente o parcial');
 const blocks=s?.offExchange?.analytics?.blocks;
 if(!blocks||blocks.symbol!==symbol||blocks.date!==date.format(now))issues.push('Bloques fuera de bolsa pendientes');
 if(!r.neuroContext?.available||r.neuroContext.symbol!==symbol||r.neuroContext.at>now||now-r.neuroContext.at>45000)issues.push('Perfil teórico de dealers pendiente o caducado');
 if(d.cvd?.status==='contradicts')issues.push('CVD contradice la confluencia');
 if(cards.some(c=>c.side==='PENDIENTE'))issues.push('Horizontes estructurales pendientes o incompletos');
 if(!r.strikeReading?.rows.length)issues.push('Delta por strike pendiente');
 if(/\b(?:NaN|Infinity|undefined)\b/.test(html))issues.push('ERROR: valor inválido en la lectura renderizada');
 const ms=performance.now()-start;if(ms>100)issues.push('Cálculo de lectura rápida superior a 100 ms');
 return {vannaEngine:vannaEngine(s,r,active,now),vanna:d.vanna,at:now,symbol,status:issues.some(x=>x.startsWith('ERROR:'))?'Error detectado':issues.length?'Revisar evidencia':'Lectura coherente',issues,ms,detail:`${r.priceNow?.headline??'Precio pendiente'} · CALL ${d.call??'—'} / PUT ${d.put??'—'} · ${d.cvd?.label??'CVD pendiente'} · ${alignment.label}. Niveles, dealers, flujo, bloques, absorción, ATR y ruta incluidos en el render completo.`};
 }catch{return {at:now,symbol,status:'Error detectado',issues:['ERROR: no se pudo evaluar la lectura completa'],detail:'La aplicación conserva sus reglas actuales.',ms:performance.now()-start};}
}
class ContinuousAudit {
 enabled=false;marketOnly=true;records:QuickAudit[]=[];error='';lastCheck=0;checks=0;lastMs=0;waiting='Pausado';private pending=false;private latest?:{s:LiveState|undefined;symbol:string;active:boolean};
 constructor(){try{const v=JSON.parse(localStorage.getItem(key)??'null');if(v?.version===1){this.enabled=v.enabled===true;this.marketOnly=v.marketOnly!==false;this.records=Array.isArray(v.records)?v.records.filter((r:QuickAudit)=>r&&Number.isFinite(r.at)&&typeof r.symbol==='string'&&typeof r.status==='string'&&typeof r.detail==='string'&&Array.isArray(r.issues)&&r.issues.every(x=>typeof x==='string')&&Number.isFinite(r.ms)).slice(-100):[];}}catch{this.error='No se pudo recuperar el historial local';}}
 save(){try{localStorage.setItem(key,JSON.stringify({version:1,enabled:this.enabled,marketOnly:this.marketOnly,records:this.records}));this.error='';}catch{this.error='No se pudo guardar el historial local';}}
 toggle(){this.enabled=!this.enabled;this.lastCheck=0;this.waiting=this.enabled?'Esperando datos':'Pausado';this.save();}
 mode(value:boolean){this.marketOnly=value;this.lastCheck=0;this.save();}
 observe(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now()){
 this.latest={s,symbol,active};if(!this.enabled){this.waiting='Pausado';return;}
 if(this.marketOnly&&!marketOpen(s,now)){this.waiting='Esperando mercado abierto y calendario disponible';return;}
 this.waiting='Activo';if(this.pending||now-this.lastCheck<30000)return;this.pending=true;
 setTimeout(()=>{this.pending=false;if(!this.enabled||!this.latest)return;const x=this.latest,at=Date.now();if(this.marketOnly&&!marketOpen(x.s,at))return;
 this.lastCheck=at;const result=auditQuick(x.s,x.symbol,x.active,at);this.checks++;this.lastMs=result.ms;
 const previous=this.records.at(-1);if(!previous||previous.symbol!==result.symbol||previous.status!==result.status||previous.issues.join('|')!==result.issues.join('|')||previous.vanna?.status!==result.vanna?.status||JSON.stringify(previous.vannaEngine?.structure.map(h=>[h.support.call.status,h.support.put.status]))!==JSON.stringify(result.vannaEngine?.structure.map(h=>[h.support.call.status,h.support.put.status]))||at-previous.at>=300000){this.records.push(result);this.records=this.records.slice(-100);this.save();}
 },0);
 }
}
export const continuousAudit=new ContinuousAudit();
export function continuousAuditPanel():string {
 const a=continuousAudit,last=a.records.at(-1);
 return `<section class="learn-panel"><div class="learn-section-head"><h2>Análisis continuo · lectura rápida</h2><button class="learn-button" data-quick-audit-toggle aria-pressed="${a.enabled}">${a.enabled?'Pausar análisis continuo':'Activar análisis continuo'}</button></div><label><input type="checkbox" data-quick-audit-market ${a.marketOnly?'checked':''}> Solo durante el mercado abierto</label><p role="status">${esc(a.waiting)} · ${a.checks} revisiones en esta apertura · último cálculo ${a.lastMs.toFixed(1)} ms</p><p>Revisión cada 30 segundos mientras Snipers esté abierto, incluso al cambiar de pantalla. Se guardan cambios de calidad y un resumen cada cinco minutos; últimos 100 registros locales. Analiza el activo seleccionado con datos disponibles.</p>${a.error?`<p class="learn-error">${esc(a.error)}</p>`:''}<p>${last?esc(last.status+' · '+last.symbol+' · '+new Date(last.at).toLocaleString('es',{timeZone:'America/New_York'})):'Sin análisis todavía'}</p>${last?`<p>${esc(last.detail)}</p><ul>${last.issues.map(x=>`<li>${esc(x)}</li>`).join('')||'<li>Sin inconsistencias detectadas en esta muestra.</li>'}</ul>`:''}<details><summary>Historial de análisis</summary>${a.records.slice(-20).reverse().map(r=>`<p>${esc(new Date(r.at).toLocaleString('es',{timeZone:'America/New_York'})+' · '+r.symbol+' · '+r.status+' · '+r.issues.join('; '))}</p>`).join('')||'<p>Sin registros.</p>'}</details><p class="learn-note">Observación local; datos pendientes no equivalen a errores de cálculo. No modifica reglas ni envía órdenes. El análisis continuo no certifica rentabilidad ni reemplaza la validación posterior.</p></section>`;
}
