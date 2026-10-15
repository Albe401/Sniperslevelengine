import {validArrivalFrame,type ArrivalFrame,type ArrivalSample} from './arrival-probability-engine';
import type {LiveState} from './live';
type Observation=ArrivalFrame&{id:string;session:string;connection:number;source:'live';outcome:'pending'|'missing'|'hit'|'miss';resolvedAt:number|null;reason:string;windows:{openMs:number;closeMs:number}[];bars:Record<string,string>};
const key='snipers.arrival-observations.v1';
/** Independent target hits; overnight time is skipped using the supplied exchange calendar. */
export class ArrivalObservationCollector {
 private rows:Observation[]=[];private clock=0;error='';
 constructor(private persist=false){if(persist)try{const saved=JSON.parse(localStorage.getItem(key)??'[]');if(Array.isArray(saved))this.rows=saved.filter(r=>validArrivalFrame(r)&&typeof r.id==='string'&&r.source==='live'&&['pending','missing','hit','miss'].includes(r.outcome)&&Array.isArray(r.windows)&&r.bars&&typeof r.bars==='object').slice(-2000).map(r=>r.outcome==='pending'?{...r,outcome:'missing',resolvedAt:null,reason:'Reinicio: continuidad no demostrada'}:r);}catch{this.error='No se recuperó el registro de llegada';}}
 snapshot(){return this.rows.map(r=>({...r,features:{...r.features},bars:{...r.bars}}));}
 samples():ArrivalSample[]{return this.snapshot().filter(r=>['hit','miss'].includes(r.outcome)).map(({connection,reason,bars,...r})=>({...r,outcome:r.outcome as 'hit'|'miss',resolvedAt:r.resolvedAt!}));}
 observe(s:LiveState|undefined,frames:ArrivalFrame[],now:number){
  if(!Number.isFinite(now)||now<=0)return;
  let changed=false;const history=s?.chartHistory,minutes=new Map((history?.minutes??[]).map(b=>[b.timestampMs,b]));
  const seen=new Set<number>(),duplicates=new Set<number>();for(const b of history?.minutes??[]){if(seen.has(b.timestampMs))duplicates.add(b.timestampMs);seen.add(b.timestampMs);}
  const signature=(b:{open:number;high:number;low:number;close:number;volume:number})=>JSON.stringify([b.open,b.high,b.low,b.close,b.volume]);
  for(const r of this.rows){if(r.symbol!==s?.symbol||r.outcome==='missing')continue;if(Object.entries(r.bars).some(([t,value])=>{const b=minutes.get(Number(t));return b&&signature(b)!==value;})){r.outcome='missing';r.reason='Vela corregida: excluir resultado';changed=true;}}
  for(const r of this.rows.filter(r=>r.outcome==='pending')){
   const missing=(reason:string)=>{r.outcome='missing';r.reason=reason;changed=true;};
   if(now<this.clock||s?.symbol!==r.symbol||s.startedMs!==r.connection||s.stocks?.phase!=='streaming'||!Number.isFinite(s.publishedMs)||s.publishedMs>now||now-s.publishedMs>3000){missing('Reloj, activo, conexión o datos sin continuidad');continue;}
   const end=Math.min(Math.floor(now/60000)*60000,r.deadline),first=Math.floor(r.at/60000)*60000;
   const sessions=r.windows.filter(v=>v.closeMs>first&&v.openMs<end).sort((a,b)=>a.openMs-b.openMs);
   if(end>first&&!sessions.length){missing('Calendario no disponible para evaluar');continue;}
   for(const session of sessions){for(let t=Math.max(first,session.openMs);t+60000<=Math.min(end,session.closeMs);t+=60000){
    const b=minutes.get(t);if(duplicates.has(t)||!b||![b.open,b.high,b.low,b.close,b.volume].every(Number.isFinite)||b.low<=0||b.volume<0||b.high<Math.max(b.open,b.close)||b.low>Math.min(b.open,b.close)){missing('Minuto ausente o inválido; no se imputa resultado');break;}
    const hash=signature(b);if(r.bars[String(t)]!==hash){r.bars[String(t)]=hash;changed=true;}
    if((b.close-r.target)*r.side>=0){r.outcome='hit';r.resolvedAt=t+60000;r.reason='Objetivo alcanzado por cierre';changed=true;break;}
   }if(r.outcome!=='pending')break;}
   if(r.outcome==='pending'&&now>=r.deadline){r.outcome='miss';r.resolvedAt=r.deadline;r.reason='No llegó por cierre antes del plazo';changed=true;}
  }
  for(const f of frames.filter(validArrivalFrame)){
   const windows=(history?.sessions??[]).filter(v=>v.closeMs>f.at&&v.openMs<f.deadline).sort((a,b)=>a.openMs-b.openMs).map(v=>({openMs:v.openMs,closeMs:v.closeMs}));
   if(f.at!==now||s?.symbol!==f.symbol||s.stocks?.phase!=='streaming'||!Number.isFinite(s.publishedMs)||s.publishedMs>now||now-s.publishedMs>3000||!windows.length||windows[0].openMs>f.at||windows.at(-1)!.closeMs<f.deadline||(f.horizon==='swing2'&&windows.length!==2))continue;
   const previous=this.rows.filter(r=>r.symbol===f.symbol&&r.side===f.side&&r.horizon===f.horizon).at(-1);
   if(previous&&(previous.outcome==='pending'||now-previous.at<1800000))continue;
   this.rows.push({...f,features:{...f.features},id:`${f.symbol}:${f.side}:${f.horizon}:${now}`,session:new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'}),connection:s?.startedMs??0,source:'live',outcome:'pending',resolvedAt:null,reason:'Objetivo congelado para evaluación',windows,bars:{}});changed=true;
  }
  this.clock=now;if(changed){this.rows=this.rows.slice(-2000);if(this.persist)try{let saved=JSON.stringify(this.rows);while(saved.length>2000000&&this.rows.length>1){this.rows.shift();saved=JSON.stringify(this.rows);}localStorage.setItem(key,saved);this.error='';}catch{this.error='Registro probabilístico en memoria: almacenamiento no disponible';}}
 }
}
