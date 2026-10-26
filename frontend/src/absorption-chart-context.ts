import type {CvdRead} from './cvd';
import {readExecutionAbsorption,type ExecutionAbsorptionView} from './execution-absorption-reading';
/** A saved reading is evaluated at capture time; it never becomes a live signal. */
export function chartAbsorptionView(read:CvdRead|undefined,symbol:string,active:boolean,now:number,capturedMs?:number):ExecutionAbsorptionView {
 if(capturedMs==null)return readExecutionAbsorption(read,symbol,active,now);
 if(!Number.isSafeInteger(capturedMs)||capturedMs<=0||!Number.isFinite(now)||capturedMs>now)return readExecutionAbsorption(undefined,symbol,false,now);
 const view=readExecutionAbsorption(read,symbol,active,capturedMs);
 const at=new Date(capturedMs).toLocaleString('es',{timeZone:'America/New_York'});
 return {...view,capturedAtMs:capturedMs,label:view.label+' · captura',detail:`Captura ${at} ET. ${view.detail} Observación guardada; no es una lectura de absorción en vivo.`};
}
