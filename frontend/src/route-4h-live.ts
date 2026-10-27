import {invoke,isTauri} from './snapshot-observation';
export interface FourHourScenario {id:string;symbol:string;at:number;side:number;target:number;targetSource:string;invalidation:number;origin:number;blockMinutes:number}
export interface FourHourProjection {horizon:string;state:string;n:number;low:number|null;typical:number|null;high:number|null;adverse:number|null;peakRegularMinutes:number|null;probabilityClaim:boolean}
export interface FourHourReview {available:boolean;mode:string;executionEnabled:boolean;reading:{version:string;symbol:string;at:number;structure:string;state:string;scenario:FourHourScenario|null;quality?:{closedBlocks:number;incompleteBlocks:number}};projections?:FourHourProjection[]|null;confirmation:{state:string};observation:{status:string;deadline:number}|null}
let cached:FourHourReview|undefined,key='',generation=0,busy=false,last=-Infinity,error='';
export function stopFourHour(){if(!key&&!cached)return;key='';cached=undefined;error='';last=-Infinity;generation++;}
/** Single request each ten seconds, only while this chart is active. No REST history download. */
export function requestFourHour(symbol:string,active:boolean,now:number,refresh?:()=>void){
 if(!active||!isTauri()){stopFourHour();return;}
 if(key!==symbol){key=symbol;cached=undefined;error='';last=-Infinity;generation++;}
 if(busy||now-last<10000)return;busy=true;last=now;const token=generation;
 void invoke<FourHourReview>('preview_route_4h',{symbol}).then(r=>{if(token===generation&&key===symbol){cached=r;error='';}}).catch(e=>{if(token===generation){cached=undefined;error=String(e).slice(0,240);}}).finally(()=>{busy=false;if(token===generation&&key===symbol)refresh?.();});
}
export function fourHourDisplay(r:FourHourReview|undefined,symbol:string,now:number){
 const pending={text:'4H · esperando estructura y precio vigentes',summary:'4H · esperando estructura y precio vigentes',scenario:undefined as FourHourScenario|undefined,projection:undefined as FourHourProjection|undefined,supported:false};
 if(!r?.available||r.mode!=='review-only'||r.executionEnabled!==false||r.reading?.version!=='route-4h-v1'||r.reading.symbol!==symbol||!Number.isFinite(r.reading.at)||r.reading.at>now||now-r.reading.at>20000)return pending;
 const structure=({up:'alcista',down:'bajista',range:'rango',pending:'pendiente'} as Record<string,string>)[r.reading.structure]??'pendiente';
 const s=r.reading.scenario,observation=r.observation;
 if(!s||s.symbol!==symbol||![1,-1].includes(s.side)||!Number.isFinite(s.at)||s.at>now||![s.origin,s.target,s.invalidation].every(v=>Number.isFinite(v)&&v>0)||(s.target-s.origin)*s.side<=0||(s.origin-s.invalidation)*s.side<=0){const q=r.reading.quality;const quality=q&&Number.isInteger(q.closedBlocks)&&Number.isInteger(q.incompleteBlocks)?` · ${q.closedBlocks} bloques cerrados · ${q.incompleteBlocks} incompletos`:'';return {...pending,text:`4H ${structure} · esperando nuevo escenario${quality}`,summary:`4H ${structure} · esperando nuevo escenario${quality}`};}
 const states:Record<string,string>={reached:'objetivo alcanzado',invalidated:'invalidado',expired:'vencido','data-gap':'seguimiento incompleto'};
 const confirmation=({supports:'apoyo intradía',supported:'apoyo intradía',divergence:'presión intradía en contra',pending:'confirmación pendiente',mixed:'apoyo mixto'} as Record<string,string>)[r.confirmation?.state]??'confirmación pendiente';
 const p=r.projections?.find(p=>p.horizon==='swing2');
 const projection=p?.state==='descriptive'&&p.probabilityClaim===false&&Number.isInteger(p.n)&&p.n>=20&&[p.low,p.typical,p.high].every(v=>typeof v==='number'&&Number.isFinite(v)&&v>0)&&p.typical===s.target&&((p.typical!-s.origin)*s.side)>0&&((p.typical!-p.low!)*s.side)>=0&&((p.high!-p.typical!)*s.side)>=0?p:undefined;
 const source=s.targetSource==='known-pivot'?'nivel':s.targetSource==='historical-median'?'mediana histórica':'rango estimado';
 const status=observation?.status&&states[observation.status];
 const history=projection?` · zona $${Math.min(projection.low!,projection.high!).toFixed(2)}–$${Math.max(projection.low!,projection.high!).toFixed(2)} · n=${projection.n} · retroceso típico $${projection.adverse?.toFixed(2)??'—'} · pico ${projection.peakRegularMinutes?.toFixed(0)??'—'} min RTH`:p?` · historial insuficiente (${p.n}/20 casos)`:' · historial pendiente';
 const text=`4H ${structure} · Swing ${s.side===1?'CALL':'PUT'} $${s.target.toFixed(2)} (${source})${history} · invalida $${s.invalidation.toFixed(2)} · ${status??confirmation}`;
 const ready=observation?.status==='pending'&&Number.isFinite(observation.deadline)&&observation.deadline>now;
 const intraday=r.projections?.find(p=>p.horizon==='session');
 const intradayText=intraday?` · Intradía histórico: ${intraday.state==='descriptive'&&intraday.probabilityClaim===false&&Number.isInteger(intraday.n)&&intraday.n>=20&&[intraday.low,intraday.typical,intraday.high].every(v=>typeof v==='number'&&Number.isFinite(v)&&v>0)&&((intraday.typical!-s.origin)*s.side)>0&&((intraday.typical!-intraday.low!)*s.side)>=0&&((intraday.high!-intraday.typical!)*s.side)>=0?'$'+intraday.typical!.toFixed(2):'muestra insuficiente'} · n=${intraday.n}`:'';
 const summary=`4H ${s.side===1?'CALL':'PUT'} · ${projection?'mediana':'referencia'} $${s.target.toFixed(2)} · ${projection?'n='+projection.n:p?'historial '+p.n+'/20':'historial pendiente'} · ${status??confirmation}`;
 return {text:text+intradayText,summary,scenario:ready&&(s.targetSource!=='historical-median'||projection)?s:undefined,projection:ready?projection:undefined,supported:r.confirmation?.state==='supported'};
}
export function currentFourHour(symbol:string,now:number){
 const reading=fourHourDisplay(cached,symbol,now);
 const reason=key!==symbol?'consulta pendiente':error?'consulta falló: '+error:!cached?busy?'calculando estructura':'sin respuesta del motor':!cached.available?'precio o conexión no vigente':cached.reading?.symbol!==symbol?'lectura de otro activo':cached.reading?.at>now?'reloj de lectura posterior':now-cached.reading?.at>20000?'lectura caducada':undefined;
 if(reason)return {...reading,text:'4H · '+reason,summary:'4H · '+reason,scenario:undefined,projection:undefined,supported:false};
 return reading;
}
export function currentFourHourReview(symbol:string,now:number){
 return cached?.available&&cached.mode==='review-only'&&cached.executionEnabled===false&&cached.reading.version==='route-4h-v1'&&cached.reading.symbol===symbol&&Number.isFinite(cached.reading.at)&&cached.reading.at<=now&&now-cached.reading.at<=20000?cached:undefined;
}
