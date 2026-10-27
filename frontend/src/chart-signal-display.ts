import {validChartSignalCoordinatorReport,type ChartSignalCoordinatorReport} from './chart-signal-coordinator';
import type {ChartSignalTicket} from './chart-signal-contract';
import {escapeHtml as esc} from './model';

export const CHART_SIGNAL_DISPLAY_POLICY=Object.freeze({version:'chart-signal-display-v1',defaultEnabled:true,qualification:'original-confirmed-source-only',experimental:true,automaticOrders:false,maxDismissed:256} as const);
export interface SignalDisplayContext {symbol:string;startedMs:number|null;publishedMs:number|null;now:number;active:boolean;currentOverlay:boolean;spot:number|null;origin:'live'|'snapshot'}
export interface DisplaySignal {id:string;state:'confirmed'|'active';ticket:Readonly<ChartSignalTicket>}
export interface SignalDisplayView {version:'chart-signal-display-v1';enabled:boolean;origin:'live'|'snapshot';symbol:string;asOfMs:number;sourcePublishedMs:number|null;freshUntilMs:number;status:string;detail:string;signals:readonly DisplaySignal[];automaticOrders:false}
const clock=(v:unknown):v is number=>typeof v==='number'&&Number.isSafeInteger(v)&&v>0&&v<=8640000000000000;
const et=(n:number)=>new Date(n).toLocaleTimeString('es',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit',hour12:false});
const money=(n:number)=>'$'+n.toFixed(2);

/** Presentation only. Never changes source tickets, frozen research policy or admission thresholds. */
export class ChartSignalDisplay {
 private identity='';private dismissed=new Set<string>();private visibleIds=new Set<string>();
 read(common:Readonly<ChartSignalCoordinatorReport>|null|undefined,c:SignalDisplayContext,enabled=true):SignalDisplayView {
  const identity=c.symbol+':'+c.startedMs;if(identity!==this.identity){this.identity=identity;this.dismissed.clear();}
  this.visibleIds.clear();
  const valid=clock(c.now)&&clock(c.publishedMs)&&clock(c.startedMs)&&c.startedMs<=c.publishedMs&&c.publishedMs<=c.now&&c.now-c.publishedMs<=3000&&c.active&&c.currentOverlay&&typeof c.spot==='number'&&Number.isFinite(c.spot)&&c.spot>0;
  const associated=valid&&!!common&&validChartSignalCoordinatorReport(common)&&common.status==='observed'&&common.symbol===c.symbol&&common.connectionStartedMs===c.startedMs&&common.publishedMs===c.publishedMs&&clock(common.evaluatedMs)&&common.evaluatedMs<=c.now&&c.now-common.evaluatedMs<=3000;
  const signals:DisplaySignal[]=[];let revised=false,expired=false,resolved='';
  if(associated)for(const row of common.signals){
   const t=row.ticket;if(!t)continue;
   if(row.revisionObserved){revised=true;continue;}
   if(!['confirmed','active'].includes(row.state)){resolved=row.state==='reached'?'Objetivo alcanzado':row.state==='invalidated'?'Señal invalidada':'Señal vencida';continue;}
   if(t.publishedMs>c.publishedMs!||t.confirmedAtMs>common.evaluatedMs!||t.confirmedAtMs>c.now||c.now>=t.deadlineMs||c.now>=t.session.closeMs){expired=true;continue;}
   if(this.dismissed.has(t.identity.id)||signals.some(s=>s.id===t.identity.id))continue;
   this.visibleIds.add(t.identity.id);if(enabled)signals.push({id:t.identity.id,state:row.state as DisplaySignal['state'],ticket:t});
  }
  const status=!enabled?'Señales CALL/PUT apagadas':!associated?'Señales CALL/PUT · sin datos vigentes':signals.length?signals.map(s=>'Posible '+s.ticket.side).join(' · '):revised?'Señal revisada · nueva confirmación pendiente':expired?'Señal vencida · esperando otra confirmación':resolved||'Señales CALL/PUT activadas · esperando confirmación';
  const reasons=associated?'Recuperación o rechazo, prueba del nivel y continuación en velas cerradas de 1 min, con apoyo y cobertura suficientes. Referencias del subyacente; no son precios de ejecución.':'La lectura debe coincidir con el activo, conexión y captura vigentes.';
  return {version:'chart-signal-display-v1',enabled,origin:c.origin,symbol:c.symbol,asOfMs:c.now,sourcePublishedMs:c.publishedMs,freshUntilMs:associated?Math.min(c.publishedMs!+3000,common.evaluatedMs!+3000,...signals.map(s=>s.ticket.deadlineMs)):c.now,status,detail:reasons,signals,automaticOrders:false};
 }
 dismiss(id:string):boolean {if(!this.visibleIds.has(id))return false;this.dismissed.add(id);while(this.dismissed.size>CHART_SIGNAL_DISPLAY_POLICY.maxDismissed)this.dismissed.delete(this.dismissed.values().next().value!);this.visibleIds.delete(id);return true;}
}

export function signalDisplayToolbar(enabled:boolean){return `<button data-chart-call-put-signals aria-pressed="${enabled}" title="Confirmaciones técnicas observadas · prueba en vivo">${enabled?'Señales CALL/PUT activadas':'Mostrar señales CALL/PUT'}</button>`;}
export interface SignalDisplayPoint {x:number;y:number;targetY:number|null;stopY:number|null}
/** Same projection for quick/detailed; arrows are placed in the bar containing original availability. */
export function signalDisplaySvg(signals:readonly DisplaySignal[],project:(s:DisplaySignal)=>SignalDisplayPoint|null,width:number,height:number,origin:'live'|'snapshot'='live'){
 if(!Number.isFinite(width)||!Number.isFinite(height)||width<200||height<140)return '';
 return signals.slice(0,2).map(s=>{
  const p=project(s);if(!p||![p.x,p.y].every(Number.isFinite)||p.x<4||p.x>width-72||p.y<12||p.y>height-14)return '';
  const t=s.ticket,color=t.side==='CALL'?'#17d6a2':'#ff6273',direction=t.side==='CALL'?-1:1,w=Math.min(280,width-90),h=88,left=Math.max(8,Math.min(width-w-74,p.x+15)),top=Math.max(34,Math.min(height-h-8,p.y+(direction<0?20:-h-20)));
  const label=`Posible ${t.side} · confirmación técnica`,detail=`${label}. Observada ${et(t.confirmedAtMs)} ET. Referencia ${money(t.entryReference!.price)}. Objetivo por cierre 1 min ${money(t.target!.price)}. Invalida por cierre 1 min ${money(t.invalidation!.price)}. Vence ${et(t.deadlineMs)} ET. ${origin==='snapshot'?'Referencia de captura':'Prueba en vivo'}.`;
  const level=(y:number|null,label:string,price:number,c:string)=>y!=null&&Number.isFinite(y)&&y>16&&y<height-10?`<path d="M ${p.x} ${y} H ${Math.min(width-74,p.x+132)}" stroke="${c}" stroke-dasharray="4 3"/><text x="${Math.min(width-142,p.x+8)}" y="${y-5}" fill="${c}" font-size="10">${label} ${money(price)}</text>`:'';
  return `<g data-chart-signal-id="${esc(s.id)}" aria-label="${esc(detail)}"><title>${esc(detail)}</title>${level(p.targetY,'Obj',t.target!.price,color)}${level(p.stopY,'Inv',t.invalidation!.price,'#ffb564')}<path d="M ${p.x} ${p.y-direction*20} V ${p.y-direction*5}" stroke="${color}" stroke-width="3"/><path d="M ${p.x} ${p.y} l -6 ${-direction*10} h 12 Z" fill="${color}"/><rect x="${left}" y="${top}" width="${w}" height="${h}" rx="6" fill="#071526" fill-opacity=".96" stroke="${color}"/><text x="${left+9}" y="${top+18}" fill="${color}" font-size="12" font-weight="700">POSIBLE ${t.side} ${t.side==='CALL'?'↑':'↓'}</text><text x="${left+9}" y="${top+35}" fill="#e2f2ff" font-size="10">Ref ${money(t.entryReference!.price)} · Obj cierre ${money(t.target!.price)}</text><text x="${left+9}" y="${top+51}" fill="#ffc783" font-size="10">Invalida cierre 1m ${money(t.invalidation!.price)}</text><text x="${left+9}" y="${top+67}" fill="#adc4d5" font-size="10">${et(t.confirmedAtMs)}–${et(t.deadlineMs)} ET · ${origin==='snapshot'?'CAPTURA':'PRUEBA EN VIVO'}</text><g data-dismiss-chart-signal="${esc(s.id)}" role="button" tabindex="0" aria-label="Ocultar esta señal ${t.side}" class="gl-call-put-close"><rect x="${left+w-25}" y="${top+3}" width="22" height="22" rx="3" fill="#18324b"/><text x="${left+w-14}" y="${top+19}" text-anchor="middle" fill="#fff" font-size="17">×</text></g></g>`;
 }).join('');
}

/** No backdating when the availability bucket is not present yet. */
export function signalKnownBar(bars:readonly number[],known:number,intervalMs:number):number|null{if(!clock(known)||!Number.isSafeInteger(intervalMs)||intervalMs<=0)return null;let lo=0,hi=bars.length;while(lo<hi){const mid=(lo+hi)>>>1;if(bars[mid]<=known)lo=mid+1;else hi=mid;}const b=bars[lo-1];return b!=null&&clock(b)&&known-b<intervalMs?b:null;}
