import {DailyTargetsCache,targetDate,type TargetRow} from './analyst-targets-cache';
import type {AtrRead} from './intraday-atr';
import {dailyDistance} from './atr-context';
import {invoke,isTauri} from './snapshot-observation';
import {escapeHtml as esc} from './model';
export interface AnalystTargets {symbol:string;low:number;average:number;high:number;date:string;source:string}
interface Report {configured:boolean;running:boolean;status:string;total:number;rows:Record<string,TargetRow>}
let storage:Storage|undefined;try{storage=globalThis.localStorage;}catch{}
const cache=new DailyTargetsCache(storage);
let report:Report|undefined,pending=false,nextCheck=0,loadError='',timer:ReturnType<typeof setTimeout>|undefined;
/** Offscreen snapshot renderer: only targets already present in the saved batch. */
export function seedCapturedAnalystTargets(rows:Record<string,TargetRow>,now:number):void {cache.merge(rows,now,false);report={configured:true,running:false,status:'Datos de la captura',total:Object.keys(rows).length,rows};}
export function validTargets(t:AnalystTargets,symbol:string):boolean {
 return t.symbol===symbol&&[t.low,t.average,t.high].every(n=>Number.isFinite(n)&&n>0)&&t.low<=t.average&&t.average<=t.high&&/^\d{4}-\d{2}-\d{2}$/.test(t.date)&&!!t.source.trim();
}
export function loadTargets(symbol:string):AnalystTargets|undefined {
 const row=cache.get(symbol);
 if(!row)return undefined;
 const t={symbol:row.symbol,low:row.low,average:row.average,high:row.high,date:new Date(row.fetchedMs).toLocaleDateString('en-CA',{timeZone:'America/New_York'}),source:'FMP'};
 return validTargets(t,symbol)?t:undefined;
}
export function analystTargetsPanel(symbol:string,current:number|null,t=loadTargets(symbol),cvdMarkup='',atr?:AtrRead):string {
 const valid=t&&validTargets(t,symbol),reference=current!=null&&Number.isFinite(current)&&current>0?current:null;
 const money=(n:number|null|undefined)=>n==null?'—':`$${n.toFixed(2)}`;
 const row=report?.rows[symbol];
 const status=valid?`FMP · consulta ${t.date} · fecha original no informada`:loadError||(!report?'Objetivos FMP pendientes':!report.configured?report.status:row?.status|| (['QQQ','SPY','IWM','DIA'].includes(symbol)?'ETF: sin objetivos de analistas por compañía':report.running?'Comprobando acceso con Basic':'Sin cobertura comprobada'));
 return `<section class="analyst-targets" aria-label="Objetivos de analistas"><div class="analyst-targets-reference">Precio del activo <b>${money(reference)}</b></div>${cvdMarkup}<div class="analyst-targets-grid">${(['low','average','high'] as const).map((name,i)=>{const target=valid?t[name]:null;const distance=target!=null&&reference!=null?(target/reference-1)*100:null;return `<article title="${esc(target!=null&&reference!=null?dailyDistance(atr,reference,target)+' · Objetivo de analistas: horizonte distinto al intradía, sin predicción de llegada.':'')}"><span>${['LOW TARGET','AVERAGE TARGET','HIGH TARGET'][i]}</span><strong>${money(target)}</strong><small class="${distance==null?'':distance>=0?'positive':'negative'}">${distance==null?'Distancia —':`${distance>=0?'+':''}${distance.toFixed(1)}%`}</small></article>`;}).join('')}</div><p class="analyst-targets-source">${esc(status)}</p>${report?.configured?`<details data-horizon-details="fmp-coverage"><summary>Cobertura Basic · ${Object.values(report.rows).filter(r=>r.status==='Disponible'&&r.fetchedMs<=Date.now()&&targetDate(r.fetchedMs)===targetDate(Date.now())).length} con objetivos</summary><p>${esc(report.status)}</p><p>${Object.values(report.rows).filter(r=>r.status==='Disponible'&&r.fetchedMs<=Date.now()&&targetDate(r.fetchedMs)===targetDate(Date.now())).map(r=>esc(r.symbol)).join(' · ')||'Todavía sin objetivos verificados'}</p></details>`:''}<p class="analyst-targets-note">Estimaciones de analistas; no confirman una entrada intradía.</p></section>`;
}
export function bindAnalystTargets(refresh:()=>void):void {
 if(!isTauri()||pending||Date.now()<nextCheck)return;
 pending=true;
 if(timer)clearTimeout(timer);
 void invoke<Report>('get_fmp_targets').then(value=>{cache.merge(value.rows);report=value;loadError='';nextCheck=Date.now()+(value.running?5000:60000);refresh();}).catch(()=>{loadError='No se pudo consultar FMP';nextCheck=Date.now()+60000;refresh();}).finally(()=>{pending=false;timer=setTimeout(()=>bindAnalystTargets(refresh),Math.max(1,nextCheck-Date.now()));});
}
