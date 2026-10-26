import type {LiveState} from './live';
import {readGexWindow,hasGexWindows,selectedGexWindow,type GexWindowMode} from './gex-windows';
/** Optional flags describe known quantities among received contracts, not missing reference series. */
export interface MassiveExposureRow {strike:number;netGex:number;callGex?:number;putGex?:number;callComplete?:boolean;putComplete?:boolean}
export interface MassiveExposureView {label?:string;spot?:number;expirations?:readonly string[];start?:string;end?:string;expected?:number;missingContracts?:number;unexpectedContracts?:number;symbol:string;mode:string;expiry:string;at:number;complete:boolean;historical:boolean;gammaUsed:number;eligible:number;net:number;rows:MassiveExposureRow[];reason:string}
/** A shared capture identity, not an independent direction vote for each consumer. */
export const massiveExposureIdentity=(v:MassiveExposureView)=>JSON.stringify(['Massive-OI',v.symbol,v.mode,v.expiry,v.at]);
const dateFormat=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'});
const validDate=(v:unknown):v is string=>typeof v==='string'&&/^\d{4}-\d{2}-\d{2}$/.test(v)&&Number.isFinite(Date.parse(v+'T12:00:00Z'))&&new Date(v+'T12:00:00Z').toISOString().slice(0,10)===v;
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v),clock=(v:unknown):v is number=>finite(v)&&Number.isSafeInteger(v)&&v>0&&v<=8640000000000000;
/** Read already-calculated Massive OI exposure. No Gexbot state, new Greeks, or provider request. */
function readLegacyExposure(s:LiveState|undefined,symbol:string,active:boolean,now:number):MassiveExposureView {
 const empty:MassiveExposureView={symbol,mode:'',expiry:'',at:0,complete:false,historical:true,gammaUsed:0,eligible:0,net:0,rows:[],reason:'Sin captura de exposición Massive del activo'};
 const fail=(reason:string)=>({...empty,reason});
 if(!clock(now)||s?.symbol!==symbol||!clock(s.publishedMs)||s.publishedMs>now)return empty;
 const snapshot=s.optionSnapshotStatus;if(snapshot?.phase!=='ready')return fail('Esperando una captura de opciones Massive');
 const horizons=Array.isArray(snapshot.horizons)?snapshot.horizons:[],h=horizons.find(h=>h?.mode==='DAY')??horizons.find(h=>h?.mode==='NEXT_SESSION');
 if(!h||horizons.filter(x=>x?.mode===h.mode).length!==1||!Array.isArray(h.expirations)||h.expirations.length!==1)return fail('Sin vencimiento individual verificable; no se suman horizontes');
 if(!clock(h.observedMs)||h.observedMs>s.publishedMs)return fail('Reloj de exposición incompatible');
 if(snapshot.observedMs!=null&&snapshot.observedMs!==h.observedMs)return fail('La exposición y la captura no comparten la misma hora');
 const expiry=h.expirations[0],sourceDay=dateFormat.format(h.observedMs);
 if(!validDate(expiry)||(h.mode==='DAY'?expiry!==sourceDay:expiry<=sourceDay))return fail('Vencimiento incompatible con la captura');
 const e=h.exposure;
 if(!e||!Number.isSafeInteger(e.gammaUsed)||!Number.isSafeInteger(e.eligible)||e.gammaUsed<=0||e.eligible<e.gammaUsed||!finite(e.netGex)||!Array.isArray(e.strikes)||!e.strikes.length)return fail('Sin gamma/OI verificables para mostrar exposición');
 if(e.strikes.length>20000)return fail('Perfil demasiado grande; no se recortan strikes silenciosamente');
 if(e.eligible>20000||[e.unknownOi,e.unsupported].some(v=>v!==undefined&&(!Number.isSafeInteger(v)||v<0||v>20000)))return fail('Conteos de cobertura de gamma inválidos');
 const rows=e.strikes as MassiveExposureRow[],strikes=new Set<number>();let sum=0,correction=0,absolute=0;
 for(const row of rows){
  if(!row||!finite(row.strike)||row.strike<=0||!finite(row.netGex)||strikes.has(row.strike))return fail('Perfil ambiguo: strikes duplicados o valores inválidos');strikes.add(row.strike);
  const {callGex:call,putGex:put}=row;
  if((call===undefined)!==(put===undefined))return fail('Componentes CALL/PUT de gamma incompletos');
  if(call!==undefined&&(!finite(call)||call<0)||put!==undefined&&(!finite(put)||put<0))return fail('Componentes CALL/PUT de gamma inválidos');
  if(call!==undefined&&put!==undefined&&Math.abs(call-put-row.netGex)>Math.max(.01,16*Number.EPSILON*Math.max(call,put)))return fail('Neto por strike incompatible con CALL menos PUT');
  const value=row.netGex-correction,next=sum+value;correction=(next-sum)-value;sum=next;absolute+=Math.abs(row.netGex);
 }
 if(!finite(sum)||!finite(absolute)||Math.abs(sum-e.netGex)>Math.max(.01,64*Number.EPSILON*rows.length*absolute))return fail('El subtotal no coincide con los strikes de esta captura');
 return {symbol,mode:h.mode,expiry,at:h.observedMs,complete:e.gexComplete===true&&e.gammaUsed===e.eligible&&(e.unknownOi??0)===0&&(e.unsupported??0)===0,historical:!active||now-s.publishedMs>15000||now-h.observedMs>300000||dateFormat.format(now)!==sourceDay,gammaUsed:e.gammaUsed,eligible:e.eligible,net:e.netGex,rows:[...rows].sort((a,b)=>a.strike-b.strike),reason:''};
}

/** Range changes are local: all windows share a single provider inventory. */
export function readMassiveExposure(s:LiveState|undefined,symbol:string,active:boolean,now:number,mode?:GexWindowMode):MassiveExposureView {
 const chosen=mode??selectedGexWindow(symbol);
 return mode||chosen!=='DAY'||hasGexWindows(s,symbol,active,now)?readGexWindow(s,symbol,active,now,chosen):readLegacyExposure(s,symbol,active,now);
}
