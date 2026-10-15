import {invoke,isTauri} from './snapshot-observation';
import type {ProbabilityReading} from './route-probability-reading';
interface Forecast {version:string;issuedMs:number;probability:number;volatilitySensitivity:[number,number];calibrated:boolean;returns:number;scope:string;case:{id:string;startMs:number;deadlineMs:number;direction:string;level:{symbol:string;horizon:string;role:string;source:string;zone:{low:number;high:number}}}}
export interface CaptureStatus {at:number;assets:Record<string,{recorded:boolean;quality?:{phase:string;streaming:boolean};arrivalBaseline?:{version:string;reason:string;calibrated:boolean;forecasts:Forecast[]}}>}
let status:CaptureStatus|undefined,started=false,busy=false;
let evaluation:{evaluatedMs:number;forecasts:number;states:Record<string,number>;metrics:{group:string;n:number;brier:number;logLoss:number;meanPredicted:number;observedArrival:number}[];errors:number}|undefined;
let nextEvaluation=0;
let evaluationError='';
/** Background IPC reads saved forecasts; rendering never creates or retrospectively changes one. */
export function startBaselineReadings(){
 if(started||!isTauri())return;started=true;
 const poll=async()=>{if(busy)return;busy=true;try{const next=await invoke<CaptureStatus>('get_research_capture_status');if(Date.now()>=nextEvaluation){nextEvaluation=Date.now()+60000;try{evaluation=await invoke('get_baseline_evaluation');evaluationError='';}catch(error){evaluation=undefined;evaluationError=String(error);}}if(JSON.stringify(next)!==JSON.stringify(status)){status=next;document.dispatchEvent(new Event('snipers:baseline-probability'));}}catch{status=undefined;}finally{busy=false;}};
 setInterval(()=>{void poll();},2000);void poll();
}
export function baselineReading(symbol:string,now:number,input:CaptureStatus|undefined=status):ProbabilityReading|undefined {
 const asset=input?.assets?.[symbol];
 if(!asset?.recorded||asset.quality?.phase!=='RTH'||!asset.quality.streaming)return;
 const baseline=asset.arrivalBaseline;
 const pending=(reason:string):ProbabilityReading=>({kind:'baseline',call:null,put:null,state:'Experimental · esperando datos',detail:reason});
 if(!baseline||baseline.version!=='log-brownian-rth-arrival-v1'||baseline.calibrated!==false)return pending('Falta el modelo experimental compatible.');
 const valid=baseline.forecasts.filter(f=>f.version===baseline.version&&f.calibrated===false&&f.case.level.symbol===symbol&&['DAY','WEEK'].includes(f.case.level.horizon)&&['up','down'].includes(f.case.direction)&&Number.isFinite(f.probability)&&f.probability>=0&&f.probability<=1&&Number.isFinite(f.issuedMs)&&f.issuedMs<=now&&now-f.issuedMs<=120000&&f.case.deadlineMs>now&&f.returns>=60);
 if(!valid.length)return pending(baseline.reason+' No se reutilizan porcentajes antiguos.');
 const choose=(h:string,d:string)=>valid.filter(f=>f.case.level.horizon===h&&f.case.direction===d).sort((a,b)=>d==='up'?a.case.level.zone.low-b.case.level.zone.low:b.case.level.zone.high-a.case.level.zone.high)[0];
 const percent=(f:Forecast|undefined)=>f?Math.round(f.probability*1000)/10:null;
 const details=(f:Forecast|undefined)=>!f?'Sin objetivo elegible':`${f.case.direction==='up'?'Superior':'Inferior'} $${f.case.level.zone.low.toFixed(2)}${f.case.level.zone.high!==f.case.level.zone.low?'–$'+f.case.level.zone.high.toFixed(2):''}: ${percent(f)}% · ${f.case.level.role} · sensibilidad σ ±25%: ${(f.volatilitySensitivity[0]*100).toFixed(1)}–${(f.volatilitySensitivity[1]*100).toFixed(1)}% (no intervalo de confianza).`;
 const up=choose('DAY','up'),down=choose('DAY','down'),wu=choose('WEEK','up'),wd=choose('WEEK','down');
 const review=evaluation?`Revisión guardada ${new Date(evaluation.evaluatedMs).toLocaleTimeString('es',{timeZone:'America/New_York'})} ET: ${evaluation.forecasts} pronósticos; estados ${JSON.stringify(evaluation.states)}; errores ${evaluation.errors}. ${evaluation.metrics.filter(m=>m.group.startsWith(symbol+':')).map(m=>`${m.group}: ${m.n} evaluables, estimación media ${(m.meanPredicted*100).toFixed(1)}%, llegada observada ${(m.observedArrival*100).toFixed(1)}%, Brier ${m.brier.toFixed(4)}, log-loss ${m.logLoss.toFixed(4)}.`).join(' ')} Son casos correlacionados, no validación independiente.`:'Revisión de resultados todavía pendiente. '+evaluationError;
 return {kind:'baseline',call:percent(up),put:percent(down),state:`Hoy · sin calibrar`,detail:`Modelo experimental de llegada, no señal CALL/PUT ni porcentaje de acierto validado. Hoy: ${details(up)} ${details(down)} Semana: ${details(wu)} ${details(wd)} Emitido ${new Date(valid[0].issuedMs).toLocaleTimeString('es',{timeZone:'America/New_York'})} ET; pronóstico congelado, actualización por minuto. ${valid[0].scope} ${review}`};
}
