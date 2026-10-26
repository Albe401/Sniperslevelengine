import type {CvdRead,BigTrade} from './cvd';
import {escapeHtml as esc} from './model';
export interface ExecutionAbsorptionCandidate {
 price:number;firstMs:number;lastMs:number;aggressiveSide:'buy'|'sell';absorbingSide:'buy'|'sell';
 trades:number;shares:number;dominancePct:number;classifiedPct:number;activityRatio:number;
 displayedSizeRatio:number;adverseExcursion:number;allowedExcursion:number;observationMs:number;
}
export interface ExecutionEvidenceRead {
 version:string;status:string;observedSinceMs:number|null;
 windows:{seconds:number;startMs:number;endMs:number;complete:boolean;classifiedPct?:number;trades?:number;quoteUpdates?:number}[];
 absorption:{version:string;status:string;reason:string;candidates:ExecutionAbsorptionCandidate[]};
}
export interface ExecutionAbsorptionView {status:'pending'|'observing'|'possible';label:string;detail:string;candidates:ExecutionAbsorptionCandidate[];capturedAtMs?:number}
const dateET=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York',year:'numeric',month:'2-digit',day:'2-digit'});
const qty=new Intl.NumberFormat('en-US',{maximumFractionDigits:0});
const timeET=new Intl.DateTimeFormat('es',{timeZone:'America/New_York',hour12:false,hour:'2-digit',minute:'2-digit',second:'2-digit'});
export function absorptionSideLabel(c:ExecutionAbsorptionCandidate){return c.absorbingSide==='buy'?'Abs. compradora':'Abs. vendedora';}
export function absorptionCandidateDetail(c:ExecutionAbsorptionCandidate){return `${absorptionSideLabel(c)} estimada @ $${c.price.toFixed(2)} · ${c.trades} operaciones / ${qty.format(c.shares)} acciones · presión ${c.aggressiveSide==='buy'?'compradora':'vendedora'} ${c.dominancePct.toFixed(0)}% del volumen clasificado · cobertura local ${c.classifiedPct.toFixed(0)}% · actividad ${c.activityRatio.toFixed(1)}× / tamaño visible ${c.displayedSizeRatio.toFixed(1)}× · ${timeET.format(c.firstMs)}–${timeET.format(c.lastMs)} ET. Indicio provisional; no confirma iceberg, giro ni entrada.`;}
function validCandidate(c:ExecutionAbsorptionCandidate,at:number):boolean {
 if(!c||![c.price,c.firstMs,c.lastMs,c.trades,c.shares,c.dominancePct,c.classifiedPct,c.activityRatio,c.displayedSizeRatio,c.adverseExcursion,c.allowedExcursion,c.observationMs].every(Number.isFinite))return false;
 return c.price>0&&Number.isSafeInteger(c.trades)&&c.trades>=5&&c.shares>0&&c.firstMs>=at-10000&&c.lastMs<=at&&c.lastMs>=c.firstMs+2000&&at-c.lastMs<=2000&&c.observationMs===at-c.firstMs&&c.dominancePct>=70&&c.dominancePct<=100&&c.classifiedPct>=80&&c.classifiedPct<=100&&c.activityRatio>=1.5&&c.displayedSizeRatio>=3&&c.allowedExcursion>=.01&&c.adverseExcursion>=0&&c.adverseExcursion<=c.allowedExcursion+1e-9&&((c.aggressiveSide==='buy'&&c.absorbingSide==='sell')||(c.aggressiveSide==='sell'&&c.absorbingSide==='buy'));
}
/** Shared quality boundary for CVD, trade cards and chart. Never changes trade/CVD direction. */
export function readExecutionAbsorption(read:CvdRead|undefined,symbol:string,active:boolean,now=Date.now()):ExecutionAbsorptionView {
 const pending=(detail:string):ExecutionAbsorptionView=>({status:'pending',label:'Absorción · pendiente',detail,candidates:[]});
 if(!Number.isFinite(now)||!active||!read||read.symbol!==symbol||read.date!==dateET.format(now)||!read.available||!Number.isFinite(read.at)||read.at>now||now-read.at>3000)return pending('Esperando operaciones y NBBO vigentes del activo.');
 const e=read.executionEvidence,a=e?.absorption,w=Array.isArray(e?.windows)?e.windows.find(w=>w?.seconds===30):undefined;
 if(!e||e.version!=='execution-evidence-v1'||!a||a.version!=='execution-absorption-v1'||!w||w.complete!==true||w.endMs!==read.at||w.startMs!==read.at-30000||e.observedSinceMs==null||!Number.isFinite(e.observedSinceMs)||e.observedSinceMs>w.startMs)return pending('Necesita 30 s observados sin interrupciones, operaciones clasificadas y tamaños bid/ask.');
 if(a.status==='pending'){
  const coverage=w.classifiedPct;
  const measured=coverage!=null&&Number.isFinite(coverage)&&coverage>=0&&coverage<=100?` Referencia de volumen clasificado en 30 s: ${coverage.toFixed(1)}% (distinta del filtro de absorción).`:'';
  const cause=a.reason==='Insufficient classified volume with visible-size coverage (minimum 80%)'?' El detector informa cobertura insuficiente de volumen clasificado con tamaños bid/ask en los últimos 10 s (mínimo 80%).':' El detector todavía espera operaciones y NBBO recientes o actividad suficiente.';
  return pending('Ventana de 30 s recibida.'+measured+cause+' Sin evidencia válida no se dibuja una zona.');
 }
 if(e.status!=='observing')return pending('El receptor todavía no confirma una secuencia causal reciente de operaciones. No se dibuja absorción con datos pendientes.');
 if(!['observing','possible-absorption'].includes(a.status)||!Array.isArray(a.candidates)||a.candidates.length>10||a.candidates.some(c=>!validCandidate(c,read.at))||(a.status==='observing'&&a.candidates.length)||(a.status==='possible-absorption'&&!a.candidates.length))return pending('Lectura incoherente; se descarta hasta recibir evidencia válida.');
 const candidates=[...a.candidates].sort((a,b)=>b.shares-a.shares);
 if(!candidates.length)return {status:'observing',label:'Absorción · sin indicio',detail:'Datos suficientes, sin presión retenida que cumpla el filtro provisional en los últimos 10 s.',candidates};
 return {status:'possible',label:absorptionSideLabel(candidates[0])+' · $'+candidates[0].price.toFixed(2),detail:'Presión intensa con avance limitado del NBBO; posible absorción, sin identidad de órdenes ni reserva oculta.',candidates};
}
export function absorptionForTrade(t:BigTrade,candidates:ExecutionAbsorptionCandidate[]){return candidates.find(c=>t.at>=c.firstMs&&t.at<=c.lastMs&&t.price>=c.price-1e-8&&t.price<c.price+.01-1e-8);}
export function executionAbsorptionPanel(view:ExecutionAbsorptionView):string {
 return `<details class="execution-absorption execution-absorption-${view.status}" data-horizon-details="execution-absorption"><summary><span>ABSORCIÓN · ACCIÓN</span><b>${esc(view.label)}</b></summary><p>${esc(view.detail)}</p>${view.candidates.map(c=>`<p class="execution-absorption-${c.absorbingSide}">${esc(absorptionCandidateDetail(c))}</p>`).join('')}<p>Compara 10 s con los 20 s anteriores. Exige ≥5 operaciones del mismo lado durante ≥2 s, cobertura ≥80%, presión de un lado ≥70%, actividad ≥1,5× y volumen ≥3× el tamaño visible mediano. El avance del punto medio NBBO se limita por spread y variación previa. Umbrales experimentales, no probabilidad. Compras agresivas retenidas sugieren absorción vendedora; ventas retenidas sugieren absorción compradora. No modifica el CVD ni la dirección de los trades. Caduca al perder evidencia; no reconstruye absorción histórica con velas.</p></details>`;
}
