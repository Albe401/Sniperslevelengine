import type {LiveState} from './live';
import type {MassiveExposureView} from './massive-exposure';
export const GEX_WINDOWS=[['DAY','0 DTE'],['WEEK_7D','1 sem'],['WEEKS_4','4 sem'],['MONTH_1','1 mes'],['MONTH_2','2 meses'],['MONTH_3','3 meses'],['MONTH_4','4 meses'],['MONTH_5','5 meses'],['MONTH_6','6 meses']] as const;
export type GexWindowMode=typeof GEX_WINDOWS[number][0];
export interface GexWindow {mode:GexWindowMode;start:string;end:string;expirations:string[];contracts:number;expected:number;missingContracts:number;unexpectedContracts:number;eligible:number;gammaUsed:number;unknownOi:number;unsupported:number;complete:boolean;callGex:number;putGex:number;netGex:number;rows:{strike:number;callGex:number;putGex:number;netGex:number}[]}
export interface GexWindowReport {version:'massive-gex-windows-v1';symbol:string;at:number;spot:number;priceAt:number|null;start:string;end:string;referenceComplete:boolean;windows:GexWindow[]}
const day=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
const finite=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n);
const count=(n:unknown):n is number=>Number.isSafeInteger(n)&&Number(n)>=0&&Number(n)<=100000;
const clock=(n:unknown):n is number=>Number.isSafeInteger(n)&&Number(n)>0&&Number(n)<=8640000000000000;
export const validGexDate=(s:unknown):s is string=>typeof s==='string'&&/^\d{4}-\d{2}-\d{2}$/.test(s)&&Number.isFinite(Date.parse(s+'T12:00:00Z'))&&new Date(s+'T12:00:00Z').toISOString().slice(0,10)===s;
export function gexWindowEnd(start:string,mode:GexWindowMode):string {
 const [y,m,d]=start.split('-').map(Number);
 if(mode.startsWith('MONTH_')){const month=m-1+Number(mode.slice(6)),last=new Date(Date.UTC(y,month+1,0)).getUTCDate();return new Date(Date.UTC(y,month,Math.min(d,last))).toISOString().slice(0,10);}
 return new Date(Date.parse(start+'T12:00:00Z')+(mode==='DAY'?0:mode==='WEEK_7D'?7:28)*86400000).toISOString().slice(0,10);
}
const choices=new Map<string,GexWindowMode>();
export const selectedGexWindow=(symbol:string):GexWindowMode=>choices.get(symbol)??'DAY';
export function selectGexWindow(symbol:string,mode:string):boolean {if(!GEX_WINDOWS.some(([m])=>m===mode))return false;choices.set(symbol,mode as GexWindowMode);if(choices.size>96)choices.delete(choices.keys().next().value!);return true;}
const reports=new Map<string,unknown>();
/** Existing background collection and Mapas share this response; no polling here. */
export function rememberGexInventory(value:unknown):void {const r=value as {symbol?:unknown;days?:unknown;gexWindows?:unknown};if(r&&typeof r.symbol==='string'&&r.days===183){reports.set(r.symbol,r.gexWindows);if(reports.size>12)reports.delete(reports.keys().next().value!);}}
const checked=new WeakMap<object,{report:GexWindowReport|null;reason:string}>();
function validate(value:unknown):{report:GexWindowReport|null;reason:string}{
 const fail=(reason:string)=>({report:null,reason});if(!value||typeof value!=='object')return fail('Esperando inventario Massive de hasta seis meses');
 const cached=checked.get(value);if(cached)return cached;
 const r=value as GexWindowReport;
 const result=()=>{
 if(r.version!=='massive-gex-windows-v1'||typeof r.symbol!=='string'||!clock(r.at)||!finite(r.spot)||r.spot<=0||r.priceAt!==null&&(!clock(r.priceAt)||r.priceAt>r.at)||!validGexDate(r.start)||r.start!==day.format(r.at)||r.end!==gexWindowEnd(r.start,'MONTH_6')||typeof r.referenceComplete!=='boolean'||!Array.isArray(r.windows)||r.windows.length!==GEX_WINDOWS.length)return fail('Inventario GEX con identidad, fechas o ventanas incompatibles');
 let totalRows=0;
 for(const [mode] of GEX_WINDOWS){const hs=r.windows.filter(w=>w?.mode===mode);if(hs.length!==1)return fail('Ventanas GEX duplicadas o ausentes');const w=hs[0];
  if(w.start!==r.start||w.end!==gexWindowEnd(r.start,mode)||!Array.isArray(w.expirations)||w.expirations.length>200||!w.expirations.every(d=>validGexDate(d)&&d>=w.start&&d<=w.end)||w.expirations.some((d,i)=>i>0&&d<=w.expirations[i-1])||![w.contracts,w.expected,w.missingContracts,w.unexpectedContracts,w.eligible,w.gammaUsed,w.unknownOi,w.unsupported].every(count)||w.contracts!==w.expected-w.missingContracts+w.unexpectedContracts||w.missingContracts>w.expected||w.unexpectedContracts>w.contracts||w.gammaUsed>w.eligible||w.eligible+w.unknownOi+w.unsupported>w.contracts||typeof w.complete!=='boolean'||!Array.isArray(w.rows)||(totalRows+=w.rows.length)>100000)return fail('Cobertura o vencimientos GEX incompatibles');
  const complete=r.referenceComplete&&w.missingContracts===0&&w.unexpectedContracts===0&&w.unknownOi===0&&w.unsupported===0&&w.eligible>0&&w.gammaUsed===w.eligible;
  if(w.complete!==complete||w.contracts>0&&!w.expirations.length||w.rows.length>w.eligible)return fail('Etiqueta de cobertura GEX contradictoria');
  let call=0,put=0,last=0;
  for(const row of w.rows){if(!row||!finite(row.strike)||row.strike<=last||![row.callGex,row.putGex,row.netGex].every(finite)||row.callGex<0||row.putGex<0||Math.abs(row.callGex-row.putGex-row.netGex)>Math.max(.01,16*Number.EPSILON*Math.max(row.callGex,row.putGex)))return fail('Perfil GEX por strike incompatible');last=row.strike;call+=row.callGex;put+=row.putGex;}
  const tolerance=Math.max(.01,64*Number.EPSILON*Math.max(1,w.rows.length)*(call+put));
  if(![w.callGex,w.putGex,w.netGex,call,put].every(finite)||w.callGex<0||w.putGex<0||Math.abs(call-w.callGex)>tolerance||Math.abs(put-w.putGex)>tolerance||Math.abs(w.callGex-w.putGex-w.netGex)>tolerance)return fail('Los totales GEX no coinciden con los strikes');
 }
 // Own immutable copy: cached validation cannot be invalidated by a consumer.
 const copy=structuredClone(r);for(const w of copy.windows){w.rows.forEach(Object.freeze);Object.freeze(w.rows);Object.freeze(w.expirations);Object.freeze(w);}Object.freeze(copy.windows);Object.freeze(copy);
 return {report:copy,reason:''};};
 const answer=result();checked.set(value,answer);return answer;
}
function source(s:LiveState|undefined,symbol:string,active:boolean,now:number):unknown {
 // A saved frame is self-contained and must never borrow newer live inventory.
 const frame=s as (LiveState&{contractChain?:{observation?:{value?:{gexWindows?:unknown}}}})|undefined;
 if(frame&&'contractChain' in frame){
  const observation=frame.contractChain?.observation as {observedMs?:number;value?:{gexWindows?:GexWindowReport;symbol?:string;days?:number}}|undefined;
  if(observation?.observedMs!==undefined&&(!clock(observation.observedMs)||observation.observedMs>now||observation.value?.gexWindows&&observation.value.gexWindows.at>observation.observedMs))return null;
  if(observation?.value?.symbol!==undefined&&observation.value.symbol!==symbol||observation?.value?.days!==undefined&&observation.value.days!==183)return null;
  return observation?.value?.gexWindows;
 }
 if(!active||s?.symbol!==symbol||!clock(s.publishedMs)||s.publishedMs>now||now-s.publishedMs>15000)return;
 return reports.get(symbol);
}
export function hasGexWindows(s:LiveState|undefined,symbol:string,active:boolean,now:number):boolean{return source(s,symbol,active,now)!==undefined;}
export function readGexWindow(s:LiveState|undefined,symbol:string,active:boolean,now:number,mode:GexWindowMode):MassiveExposureView {
 const empty:MassiveExposureView={symbol,mode,expiry:'',at:0,complete:false,historical:true,gammaUsed:0,eligible:0,net:0,rows:[],reason:'Esperando inventario Massive de hasta seis meses',label:GEX_WINDOWS.find(([m])=>m===mode)?.[1]??mode};
 if(!clock(now)||s?.symbol!==symbol||!clock(s.publishedMs)||s.publishedMs>now)return {...empty,reason:'Activo o reloj de publicación incompatible'};
 const parsed=validate(source(s,symbol,active,now));if(!parsed.report)return {...empty,reason:parsed.reason};const r=parsed.report;
 if(r.symbol!==symbol||r.at>now)return {...empty,reason:'Inventario GEX de otro activo o fecha futura'};
 const w=r.windows.find(w=>w.mode===mode)!;
 const view={...empty,expiry:w.end,at:r.at,spot:r.spot,expirations:w.expirations,start:w.start,end:w.end,expected:w.expected,missingContracts:w.missingContracts,unexpectedContracts:w.unexpectedContracts,complete:w.complete,historical:!active||now-s.publishedMs>15000||now-r.at>300000||day.format(now)!==r.start||r.priceAt==null||now-r.priceAt>120000||day.format(r.priceAt)!==r.start,gammaUsed:w.gammaUsed,eligible:w.eligible,net:w.netGex,rows:w.rows,reason:''};
 if(w.contracts===0)return {...view,reason:mode==='DAY'?'Sin contratos 0 DTE en esta captura; no se sustituye otro vencimiento':w.expected?'Faltan los contratos esperados de este plazo':'Sin vencimientos en este plazo'};
 if(w.gammaUsed===0)return {...view,reason:'Sin gamma/OI verificables para este plazo'};
 return view;
}
export function gexWindowComparisons(s:LiveState|undefined,symbol:string,active:boolean,now:number):MassiveExposureView[]{return hasGexWindows(s,symbol,active,now)?GEX_WINDOWS.map(([mode])=>readGexWindow(s,symbol,active,now,mode)):[];}
