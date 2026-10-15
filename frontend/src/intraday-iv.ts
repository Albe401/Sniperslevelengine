export interface IvObservation {key:string;at:number;percent:number}
export interface IvContext {percent:number;trend:string;delta:number|null;context:string;tone:string}
export class IvTracker {
 private last?:IvObservation;
 private result?:IvContext;
 update(observation:IvObservation|undefined):IvContext|undefined {
  if(!observation){this.last=undefined;this.result=undefined;return;}
  const previous=this.last;
  if(previous?.key===observation.key&&observation.at<=previous.at)return this.result;
  const comparable=previous?.key===observation.key&&observation.at>previous.at&&observation.at-previous.at<=15*60000;
  const delta=comparable?observation.percent-previous.percent:null;
  const trend=delta==null?'ESTABLECIENDO LECTURA':delta>=.15-1e-9?'EXPANDIENDO':delta<=-.15+1e-9?'CONTRAYENDO':'ESTABLE';
  this.last=observation;
  return this.result={percent:observation.percent,delta,trend,tone:trend==='EXPANDIENDO'?'yellow':trend==='CONTRAYENDO'?'cyan':'purple',context:delta==null?'IV ATM: esperando segunda observación':trend==='EXPANDIENDO'?'IV expandiendo · mayor expectativa de magnitud':trend==='CONTRAYENDO'?'IV contrayendo · menor expectativa de magnitud':'IV estable · magnitud sin cambio claro'};
 }
}
export function rangeNarrative(control:string,range:{remainingCall:number|null;remainingPut:number|null;extensionCall:number|null;extensionPut:number|null}|undefined,move:number|undefined):string {
 if(!range)return '';
 const call=control==='COMPRADORES';
 if(!call&&control!=='VENDEDORES')return 'Control en equilibrio · rango como contexto';
 const side=call?'CALL':'PUT',extension=call?range.extensionCall:range.extensionPut,remaining=call?range.remainingCall:range.remainingPut;
 if(extension!=null&&extension>0)return `Rango ${side} extendido +$${extension.toFixed(2)}`;
 if(remaining==null||move==null||move<=0)return `Recorrido ${side} sin medida completa`;
 const ratio=remaining/move;
 return ratio<=.15?`Rango ${side} casi consumido`:ratio<=.35?`Queda poco recorrido ${side} esperado`:`Hay recorrido ${side} disponible`;
}
