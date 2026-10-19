import {invoke,isTauri} from './snapshot-observation';
import {escapeHtml as esc} from './model';
export interface ClockReport {checkedMs:number;source:string;serviceStatusReadable:boolean;samples:number;medianOffsetMs:number|null;spreadMs:number|null;state:string;detail:string}
let report:ClockReport|undefined,busy=false,error='';
export function clockContent(r:ClockReport|undefined,loading:boolean,message:string,available:boolean):string {
 const number=(n:number|null|undefined)=>n!=null&&Number.isFinite(n)?`${n.toFixed(1)} ms`:'Sin medida';
 const status:Record<string,string>={OFFSET_DETECTED:'DESFASE DETECTADO',UNSTABLE:'COMPARACIÓN INESTABLE',UNVERIFIED:'SIN VERIFICAR',SMALL_OBSERVED_OFFSET:'MEDICIÓN PUNTUAL'};
 return `<section class="panel connection-panel"><div class="panel-head"><div><span class="eyebrow">CALIDAD DE TIEMPO</span><h2>Reloj del equipo</h2></div><span class="tag">${loading?'COMPROBANDO':r?status[r.state]??'SIN VERIFICAR':'SIN CONSULTAR'}</span></div><p>Compara tres muestras con time.windows.com. Consulta de solo lectura: no ajusta Windows ni modifica los timestamps del mercado.</p><button class="replay-button" data-clock-check ${loading||!available?'disabled':''}>${loading?'Comprobando…':'Comprobar reloj'}</button>${message?`<p class="panel-caption" role="status">${esc(message)}</p>`:''}${r?`<div class="status-line"><span>Desfase mediano observado</span><strong>${number(r.medianOffsetMs)}</strong></div><div class="status-line"><span>Variación entre muestras</span><strong>${number(r.spreadMs)}</strong></div><div class="status-line"><span>Muestras recibidas</span><strong>${r.samples} / 3</strong></div><div class="status-line"><span>Consulta al servicio Windows</span><strong>${r.serviceStatusReadable?'Disponible':'No disponible'}</strong></div><p class="panel-caption">${esc(r.detail)}<br>Fuente: ${esc(r.source)} · consulta ${esc(new Date(r.checkedMs).toLocaleString('es'))}. El signo corresponde al desfase informado por Windows. Un fallo NTP no demuestra que el reloj esté bien ni mal. Esta lectura puntual no certifica latencia de Data.</p>`:'<p class="panel-caption">El monitor rechaza eventos con fecha futura. Un reloj desajustado puede impedir acreditar datos recientes. Disponible en la aplicación Windows.</p>'}</section>`;
}
export const clockDiagnosticPanel=()=>`<div id="clock-diagnostic">${clockContent(report,busy,error,isTauri())}</div>`;
function paint(){const root=document.querySelector('#clock-diagnostic');if(root){root.innerHTML=clockContent(report,busy,error,isTauri());bindClockDiagnostic();}}
export function bindClockDiagnostic(){const button=document.querySelector<HTMLButtonElement>('[data-clock-check]');if(button)button.onclick=async()=>{
 if(busy||!isTauri())return;
 busy=true;report=undefined;error='';paint();
 try{report=await invoke<ClockReport>('get_system_clock_diagnostic');}catch(e){error=String(e);}finally{busy=false;paint();}
};}
