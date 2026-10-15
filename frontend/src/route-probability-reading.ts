import {predictRouteProbability,type ProbabilityModel} from './route-probability-model';
import {routeProbabilityCollector,type ProbabilitySample} from './route-probability';
import artifacts from './route-probability-models.json';
const models=artifacts as ProbabilityModel[];
export interface ProbabilityReading {call:number|null;put:number|null;detail:string;state:string;kind?:'baseline';provisional?:{call:number;put:number}}
export interface VisibleRouteReading extends ProbabilityReading {provisional?:{call:number;put:number}}
/** Provisional observations remain explicitly separate from calibrated probabilities. */
export function visibleRouteReading(reading:ProbabilityReading,score:{call:number|null;put:number|null}):VisibleRouteReading {
 return reading.kind==='baseline'||reading.provisional||reading.call!=null||reading.put!=null||score.call==null||score.put==null?reading:{...reading,provisional:{call:score.call,put:score.put}};
}
export function routePercentLabel(reading:VisibleRouteReading|undefined,direction:number):string {
 const percent=direction>0?reading?.call:reading?.put;
 const provisional=direction>0?reading?.provisional?.call:reading?.provisional?.put;
 return percent!=null?`P(ref) ${percent}% / 10 min`:provisional!=null?`Conf. ${provisional}% · provisional`:'P(ref) pendiente';
}
export function routeProbabilityReading(symbol:string,now:number,routeKey?:string):ProbabilityReading {
 const row=routeProbabilityCollector.current(symbol,now);
 if(!row||routeKey!=null&&row.id!==routeKey)return {call:null,put:null,state:'Esperando escenario evaluable',detail:'Resultado: siguiente referencia por cierre antes de invalidarse, dentro de 10 minutos. Sin un escenario activo comparable no se publica una probabilidad.'};
 return probabilityReading(row,models.find(m=>m.symbol===symbol),now);
}
export function probabilityReading(row:ProbabilitySample,model:ProbabilityModel|undefined,now:number):ProbabilityReading {
 // A forecast describes the frozen starting scenario, not a newly recalculated tick-by-tick probability.
 const estimate=predictRouteProbability(row,model,row.at);
 if(now<row.at||now>row.deadline||row.outcome!=='pending')return {call:null,put:null,state:'Escenario terminado',detail:'Esperar una nueva secuencia.'};
 return {provisional:estimate.percent==null?row.observedConfluence:undefined,call:row.direction===1?estimate.percent:null,put:row.direction===-1?estimate.percent:null,state:estimate.percent==null?'Probabilidad pendiente de validación':`Escenario ${row.direction>0?'CALL':'PUT'} · 10 min`,detail:`${estimate.reason} Referencia $${row.target.toFixed(2)}; invalida por cierre ${row.direction>0?'bajo':'sobre'} $${row.stop.toFixed(2)}. Emitida ${new Date(row.at).toLocaleTimeString('es',{timeZone:'America/New_York'})} ET.${estimate.n?` Banda validada: ${estimate.n} resultados; intervalo 95% de frecuencia ${estimate.lower}–${estimate.upper}%.`:''}${routeProbabilityCollector.error?' '+routeProbabilityCollector.error:''}`};
}
