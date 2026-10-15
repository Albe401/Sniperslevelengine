import type {Side} from './overnight-types';
import type {ChartHistory} from './candle-series';
export interface OvernightObservation {version:'overnight-v1';date:string;symbol:string;bias:Side;scenario:string;buyPct:number|null;classifiedCoverage:number;close:number|null;nextDate?:string;premarketGapPct?:number|null;premarketState?:string;open?:number|null;afterOpen30m?:number|null;outcome?:'HIT'|'MISS'|'FLAT'|'PENDING'}
export interface ValidationRead {evaluated:number;hits:number;misses:number;flat:number;hitRate:number|null;calibrated:false;reason:string}
/** Descriptive only. No in-sample win rate is promoted to probability. */
export function readValidation(rows:OvernightObservation[],symbol='QQQ'):ValidationRead{
 const unique=new Map<string,OvernightObservation>();
 for(const r of rows)if(r.version==='overnight-v1'&&r.symbol===symbol&&/^\d{4}-\d{2}-\d{2}$/.test(r.date)&&!unique.has(r.date))unique.set(r.date,r);
 const outcomes=[...unique.values()].filter(r=>r.bias==='BUY'||r.bias==='SELL').map(r=>r.outcome);
 const hits=outcomes.filter(o=>o==='HIT').length,misses=outcomes.filter(o=>o==='MISS').length,flat=outcomes.filter(o=>o==='FLAT').length,evaluated=hits+misses+flat;
 return {evaluated,hits,misses,flat,hitRate:evaluated>=20?100*hits/evaluated:null,calibrated:false,reason:evaluated<20?'Muestra insuficiente para mostrar tasa; requiere resultados reales y validación fuera de muestra':'Tasa descriptiva; aún falta partición temporal fuera de muestra y calibración'};
}
export function gradeNextOpen(record:OvernightObservation,nextDate:string,open:number,afterOpen30m:number):OvernightObservation{
 if(record.bias==='PENDING'||record.bias==='NEUTRAL'||!record.close||![open,afterOpen30m].every(Number.isFinite)||open<=0||afterOpen30m<=0||nextDate<=record.date)return {...record,outcome:'PENDING'};
 const move=afterOpen30m/open-1,threshold=.001;
 return {...record,nextDate,open,afterOpen30m,outcome:Math.abs(move)<threshold?'FLAT':(record.bias==='BUY')===(move>0)?'HIT':'MISS'};
}
export function gradeFromHistory(record:OvernightObservation,history:ChartHistory|undefined):OvernightObservation{
 const session=history?.sessions.filter(s=>s.date>record.date&&s.openMs<s.closeMs).sort((a,b)=>a.openMs-b.openMs)[0];if(!session)return record;
 const bars=history!.minutes.filter(b=>b.timestampMs>=session.openMs&&b.timestampMs<session.openMs+30*60000&&Number.isFinite(b.open)&&Number.isFinite(b.close)&&b.open>0&&b.close>0).sort((a,b)=>a.timestampMs-b.timestampMs);
 if(bars.length!==30||bars[0].timestampMs!==session.openMs||bars[29].timestampMs!==session.openMs+29*60000||new Set(bars.map(b=>b.timestampMs)).size!==30)return record;
 return gradeNextOpen(record,session.date,bars[0].open,bars[29].close);
}
