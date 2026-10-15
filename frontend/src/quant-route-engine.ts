/** Independent research engine. No imports from admission, execution or legacy scorers. */
export const quantVersion='quant-route-close-first-10m-v1';
export const quantHorizon=600000;
export type QuantOutcome='upper'|'lower'|'neither'|'missing'|'pending';
export interface QuantFrame {
 version:typeof quantVersion;symbol:string;session:string;connection:number;at:number;closeMs:number;
 spot:number;upper:number;lower:number;atr:number;features:Record<string,number|null>;ready:boolean;reason:string;
}
export interface QuantSample extends QuantFrame {id:string;deadline:number;outcome:QuantOutcome;resolvedAt:number|null;source:'live'|'synthetic';observedBars?:Record<string,string>}
export interface QuantBar {timestampMs:number;open:number;high:number;low:number;close:number;volume:number}
export interface QuantModel {
 version:string;symbol:string;source:'live'|'synthetic';columns:string[];means:number[];scales:number[];min:number[];max:number[];present:number[];
 weights:number[][];intercepts:number[];temperature:number;trainedThrough:number;calibratedThrough:number;validatedThrough:number;
 counts:{train:number;calibration:number;test:number;sessions:number};brier:number;baselineBrier:number;logLoss:number;baselineLogLoss:number;
 bands:{winner:number;bin:number;n:number;predicted:number;observed:number;lower:number;upper:number}[];
}
export interface QuantReading {state:'waiting'|'collecting'|'neutral'|'signal';label:string;reason:string;probabilities:number[]|null;side:number;upper:number|null;lower:number|null;sampleId:string;at:number}
export const quantClasses=['upper','lower','neither'] as const;
export const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v);
const mean=(v:number[])=>v.reduce((a,b)=>a+b,0)/v.length;
export function quantSoftmax(z:number[]):number[]{const peak=Math.max(...z),e=z.map(v=>Math.exp(v-peak)),sum=e.reduce((a,b)=>a+b,0);return e.map(v=>v/sum);}
const day=(t:number)=>finite(t)&&t>0?new Date(t).toLocaleDateString('en-CA',{timeZone:'America/New_York'}):'';
export function validQuantSample(v:unknown):v is QuantSample {
 const r=v as QuantSample;
 return !!r&&r.version===quantVersion&&['live','synthetic'].includes(r.source)&&typeof r.id==='string'&&typeof r.symbol==='string'&&r.session===day(r.at)&&
 [r.at,r.deadline,r.closeMs,r.connection,r.spot,r.upper,r.lower,r.atr].every(finite)&&r.at>0&&r.deadline-r.at===quantHorizon&&r.closeMs<=r.at&&r.lower>0&&r.lower<r.spot&&r.spot<r.upper&&r.atr>0&&
 !!r.features&&Object.values(r.features).every(v=>v===null||finite(v))&&['pending','upper','lower','neither','missing'].includes(r.outcome)&&
 (r.outcome==='pending'?r.resolvedAt===null:finite(r.resolvedAt)&&r.resolvedAt>r.at&&r.resolvedAt<=r.deadline);
}
const encode=(f:Record<string,number|null>,m:Pick<QuantModel,'columns'|'means'|'scales'>)=>m.columns.flatMap((k,i)=>finite(f[k])?[(f[k]!-m.means[i])/m.scales[i],0]:[0,1]);
const logits=(f:Record<string,number|null>,m:QuantModel)=>{const x=encode(f,m);return m.weights.map((w,k)=>w.reduce((z,v,i)=>z+v*x[i],m.intercepts[k]));};
export function quantEstimate(f:Record<string,number|null>,m:QuantModel){return quantSoftmax(logits(f,m).map(z=>z/m.temperature));}
function wilson(wins:number,n:number){const p=wins/n,z2=1.96**2,den=1+z2/n,c=(p+z2/(2*n))/den,h=1.96*Math.sqrt(p*(1-p)/n+z2/(4*n*n))/den;return {lower:c-h,upper:c+h};}
/** Fixed ridge penalty, chronological 60/20/20 session split, purged overlap. Never trains in the live UI. */
export function trainQuantRoute(input:unknown[],symbol:string){
 const candidates=input.filter(validQuantSample).filter(r=>r.symbol===symbol&&quantClasses.includes(r.outcome as typeof quantClasses[number])).sort((a,b)=>a.at-b.at);
 const rows:QuantSample[]=[];let last=-Infinity;const ids=new Set<string>();
 for(const r of candidates){if(ids.has(r.id)||r.at-last<quantHorizon)continue;ids.add(r.id);rows.push(r);last=r.at;}
 const sessions=[...new Set(rows.map(r=>r.session))].sort(),counts:Record<string,number>={samples:rows.length,sessions:sessions.length};
 const fail=(reason:string)=>({reason,counts,publishable:false,model:undefined as QuantModel|undefined});
 if(sessions.length<30)return fail('Faltan 30 sesiones completas del nuevo resultado cuantitativo');
 if(new Set(rows.map(r=>r.source)).size!==1)return fail('No mezclar datos reales y sintéticos');
 const calDay=sessions[Math.floor(sessions.length*.6)],testDay=sessions[Math.floor(sessions.length*.8)];
 const calAt=rows.find(r=>r.session===calDay)!.at,testAt=rows.find(r=>r.session===testDay)!.at;
 const train=rows.filter(r=>r.session<calDay&&r.deadline<calAt),cal=rows.filter(r=>r.session>=calDay&&r.session<testDay&&r.deadline<testAt),test=rows.filter(r=>r.session>=testDay);
 Object.assign(counts,{train:train.length,calibration:cal.length,test:test.length});
 if(train.length<300||cal.length<100||test.length<100)return fail('Muestra temporal insuficiente (mínimos 300/100/100)');
 if([train,cal,test].some(part=>quantClasses.some(k=>part.filter(r=>r.outcome===k).length<10)))return fail('Faltan resultados de las tres clases en algún bloque');
 const columns=Object.keys(train[0].features).sort();
 if(!columns.length||rows.some(r=>Object.keys(r.features).sort().join('|')!==columns.join('|')))return fail('Esquemas de variables incompatibles');
 const values=columns.map(k=>train.map(r=>r.features[k]).filter(finite)),means=values.map(v=>v.length?mean(v):0),scales=values.map((v,i)=>v.length?Math.max(1e-6,Math.sqrt(mean(v.map(x=>(x-means[i])**2)))):1);
 const m:QuantModel={version:quantVersion,symbol,source:rows[0].source,columns,means,scales,min:values.map(v=>v.length?Math.min(...v):0),max:values.map(v=>v.length?Math.max(...v):0),present:values.map(v=>v.length),weights:Array.from({length:3},()=>Array(columns.length*2).fill(0)),intercepts:[0,0,0],temperature:1,trainedThrough:Math.max(...train.map(r=>r.deadline)),calibratedThrough:Math.max(...cal.map(r=>r.deadline)),validatedThrough:Math.max(...test.map(r=>r.deadline)),counts:{train:train.length,calibration:cal.length,test:test.length,sessions:sessions.length},brier:0,baselineBrier:0,logLoss:0,baselineLogLoss:0,bands:[]};
 const xs=train.map(r=>encode(r.features,m)),ys=train.map(r=>quantClasses.indexOf(r.outcome as typeof quantClasses[number]));
 for(let epoch=0;epoch<400;epoch++){
  const gradients=m.weights.map(w=>w.map(()=>0)),bias=[0,0,0];
  xs.forEach((x,j)=>{const p=quantSoftmax(m.weights.map((w,k)=>w.reduce((z,v,i)=>z+v*x[i],m.intercepts[k])));for(let k=0;k<3;k++){const error=p[k]-(ys[j]===k?1:0);bias[k]+=error;for(let i=0;i<x.length;i++)gradients[k][i]+=error*x[i];}});
  for(let k=0;k<3;k++){m.intercepts[k]-=.05*bias[k]/train.length;for(let i=0;i<m.weights[k].length;i++)m.weights[k][i]-=.05*(gradients[k][i]/train.length+.02*m.weights[k][i]);}
 }
 // Temperature calibrated only on the middle block. Test remains untouched.
 let best=Infinity;for(const temperature of [.5,.75,1,1.5,2,3,5]){const loss=mean(cal.map(r=>-Math.log(Math.max(1e-12,quantSoftmax(logits(r.features,m).map(z=>z/temperature))[quantClasses.indexOf(r.outcome as typeof quantClasses[number])]))));if(loss<best){best=loss;m.temperature=temperature;}}
 const baseline=quantClasses.map(k=>train.filter(r=>r.outcome===k).length/train.length),predictions=test.map(r=>quantEstimate(r.features,m));
 const brier=(p:number[],r:QuantSample)=>p.reduce((s,v,k)=>s+(v-(r.outcome===quantClasses[k]?1:0))**2,0);
 m.brier=mean(test.map((r,i)=>brier(predictions[i],r)));m.baselineBrier=mean(test.map(r=>brier(baseline,r)));
 m.logLoss=mean(test.map((r,i)=>-Math.log(Math.max(1e-12,predictions[i][quantClasses.indexOf(r.outcome as typeof quantClasses[number])]))));m.baselineLogLoss=mean(test.map(r=>-Math.log(baseline[quantClasses.indexOf(r.outcome as typeof quantClasses[number])])));
 for(let k=0;k<3;k++)for(let bin=6;bin<10;bin++){const indexes=predictions.map((p,i)=>({p,i})).filter(({p})=>p[k]>=.6&&p[k]-Math.max(...p.filter((_,j)=>j!==k))>=.15&&Math.min(9,Math.floor(p[k]*10))===bin).map(v=>v.i),n=indexes.length;if(!n)continue;const wins=indexes.filter(i=>test[i].outcome===quantClasses[k]).length;m.bands.push({winner:k,bin,n,predicted:mean(indexes.map(i=>predictions[i][k])),observed:wins/n,...wilson(wins,n)});}
 const supported=m.bands.some(b=>b.n>=100&&b.upper-b.lower<=.2&&Math.abs(b.predicted-b.observed)<=.1);
 const publishable=m.source==='live'&&m.brier<m.baselineBrier&&m.logLoss<m.baselineLogLoss&&supported;
 return {model:m,counts,publishable,reason:publishable?'Mejora frente a tasa base en prueba futura; bandas de decisión limitadas':'Candidato de investigación: sintético, sin mejora o sin banda respaldada; no publicar señales'};
}
export function readQuantModel(frame:QuantFrame,model:QuantModel|undefined):QuantReading {
 const out:QuantReading={state:frame.ready?'collecting':'waiting',label:'CUANTITATIVO · esperando validación',reason:frame.reason,probabilities:null,side:0,upper:frame.ready?frame.upper:null,lower:frame.ready?frame.lower:null,sampleId:'',at:frame.at};
 if(!frame.ready)return out;
 if(!validQuantSample({...frame,id:'validation',deadline:frame.at+quantHorizon,outcome:'pending',resolvedAt:null,source:'live'})){out.state='waiting';out.reason='Escenario cuantitativo inválido';return out;}
 out.reason='Recolectando escenarios reales; sin modelo validado del nuevo resultado a 10 min';
 if(!model)return out;
 const m=model;
 const arrays=[m.columns,m.means,m.scales,m.min,m.max,m.present,m.weights,m.intercepts,m.bands];
 if(!arrays.every(Array.isArray)||!m.counts||Object.values(m.counts).some(v=>!Number.isInteger(v)||v<=0)||!finite(m.temperature)||m.temperature<=0||m.version!==quantVersion||m.symbol!==frame.symbol||m.source!=='live'||![m.trainedThrough,m.calibratedThrough,m.validatedThrough,m.brier,m.baselineBrier,m.logLoss,m.baselineLogLoss].every(finite)||!(m.trainedThrough<m.calibratedThrough&&m.calibratedThrough<m.validatedThrough&&m.validatedThrough<frame.at)||frame.at-m.validatedThrough>30*86400000||m.brier>=m.baselineBrier||m.logLoss>=m.baselineLogLoss||m.counts.train<300||m.counts.calibration<100||m.counts.test<100||m.counts.sessions<30){out.reason='Modelo incompatible, sintético, vencido o sin mejora temporal';return out;}
 if(!m.columns.length||m.columns.join('|')!==Object.keys(frame.features).sort().join('|')||[m.means,m.scales,m.min,m.max,m.present].some(v=>v.length!==m.columns.length)||m.weights.length!==3||m.intercepts.length!==3||m.weights.some(w=>!Array.isArray(w)||w.length!==m.columns.length*2)||[...m.means,...m.scales,...m.min,...m.max,...m.present,...m.intercepts,...m.weights.flat()].some(v=>!finite(v))||m.scales.some(v=>v<=0)){out.reason='Modelo inválido';return out;}
 for(let i=0;i<m.columns.length;i++){const v=frame.features[m.columns[i]],margin=Math.max(m.scales[i],(m.max[i]-m.min[i])*.2);if(finite(v)&&(m.present[i]===0||v<m.min[i]-margin||v>m.max[i]+margin)){out.reason='Escenario fuera del rango entrenado';return out;}}
 const p=quantEstimate(frame.features,m);
 if(p.length!==3||p.some(v=>!finite(v)||v<0||v>1)||Math.abs(p.reduce((a,b)=>a+b,0)-1)>1e-9){out.reason='Cálculo probabilístico inválido; señal suspendida';return out;}
 const winner=p.indexOf(Math.max(...p)),matchingBands=m.bands.filter(b=>b.winner===winner&&b.bin===Math.min(9,Math.floor(p[winner]*10))),band=matchingBands.length===1?matchingBands[0]:undefined;
 if(p[winner]<.6||p[winner]-Math.max(...p.filter((_,k)=>k!==winner))<.15){out.state='neutral';out.label='CUANTITATIVO · sin predominio';out.reason='La distribución no supera el umbral experimental de decisión';return out;}
 if(!band||!Number.isInteger(band.n)||band.n<100||![band.predicted,band.observed,band.lower,band.upper].every(v=>finite(v)&&v>=0&&v<=1)||band.upper<band.lower||band.observed<band.lower-1e-12||band.observed>band.upper+1e-12||band.upper-band.lower>.2||Math.abs(band.predicted-band.observed)>.1){out.reason='Franja del porcentaje sin respaldo suficiente';return out;}
 out.probabilities=p;out.side=winner===0?1:winner===1?-1:0;out.state=out.side?'signal':'neutral';out.label=out.side?`CUANTITATIVO ${out.side>0?'↑ CALL':'↓ PUT'} · experimental`:'CUANTITATIVO · sin llegada prevista';out.reason=`Primer cierre en referencia superior/inferior, o ninguna, en 10 min. Banda: ${band.n} casos posteriores. No estima rentabilidad de opciones.`;return out;
}
/** Freeze levels/features at observation; continuity gaps invalidate, never fabricate outcomes. */
export class QuantRouteEngine {
 private samples:QuantSample[]=[];private lastAt=0;private lastStart=new Map<string,number>();
 error='';
 constructor(private persist=false,private storageKey='snipers.quant-route.v1'){if(persist)try{const saved=JSON.parse(localStorage.getItem(this.storageKey)??'[]');if(Array.isArray(saved))this.samples=saved.filter(validQuantSample).slice(-4000).map(r=>r.outcome==='pending'?{...r,outcome:'missing',resolvedAt:r.at+1,reason:'Reinicio: continuidad no demostrada'}:r);for(const r of this.samples)this.lastStart.set(r.symbol,Math.max(this.lastStart.get(r.symbol)??0,r.at));}catch{this.error='Historial cuantitativo no recuperado';}}
 snapshot(){return this.samples.map(r=>({...r,features:{...r.features},observedBars:r.observedBars?{...r.observedBars}:undefined}));}
 observe(f:QuantFrame,bars:QuantBar[],model?:QuantModel,allowStart=true):QuantReading {
  if(!finite(f.at)||f.at<=0)return readQuantModel({...f,ready:false,reason:'Reloj inválido'},undefined);
  let changed=false;
  const signature=(b:QuantBar)=>JSON.stringify([b.open,b.high,b.low,b.close,b.volume]);
  const received=new Map(bars.map(b=>[String(b.timestampMs),signature(b)]));
  for(const r of this.samples){if(r.symbol!==f.symbol||r.session!==f.session||r.outcome==='missing')continue;if(Object.entries(r.observedBars??{}).some(([t,value])=>received.has(t)&&received.get(t)!==value)){r.outcome='missing';r.resolvedAt=Math.max(r.at+1,Math.min(f.at,r.deadline));r.reason='Vela observada corregida: resultado excluido';changed=true;}}
  for(const r of this.samples.filter(r=>r.outcome==='pending')){
   const miss=(reason:string)=>{r.outcome='missing';r.resolvedAt=Math.max(r.at+1,Math.min(f.at,r.deadline));r.reason=reason;changed=true;};
   if(f.at<this.lastAt||!f.ready||r.symbol!==f.symbol||r.session!==f.session||r.connection!==f.connection){miss('Cambio de sesión/conexión o datos no vigentes');continue;}
   const first=Math.floor(r.at/60000)*60000,last=Math.min(Math.floor(f.at/60000)*60000,r.deadline),expected=Math.max(0,Math.floor((last-first)/60000));
   const relevant=bars.filter(b=>b.timestampMs>=first&&b.timestampMs+60000<=last).sort((a,b)=>a.timestampMs-b.timestampMs);
   if(relevant.length!==expected||relevant.some((b,i)=>b.timestampMs!==first+i*60000||![b.open,b.high,b.low,b.close,b.volume].every(finite)||b.low<=0||b.volume<0||b.high<Math.max(b.open,b.close)||b.low>Math.min(b.open,b.close))){miss('Hueco, duplicado o vela inválida en el resultado');continue;}
   for(const b of relevant){r.observedBars??={};const key=String(b.timestampMs);if(r.observedBars[key]!==signature(b)){r.observedBars[key]=signature(b);changed=true;}if(b.close>=r.upper||b.close<=r.lower){r.outcome=b.close>=r.upper?'upper':'lower';r.resolvedAt=b.timestampMs+60000;changed=true;break;}}
   if(r.outcome==='pending'&&f.at>=r.deadline){r.outcome='neither';r.resolvedAt=r.deadline;changed=true;}
  }
  const current=this.samples.find(r=>r.outcome==='pending'&&r.symbol===f.symbol),reading=readQuantModel(current??f,model);
  if(allowStart&&f.ready&&!current&&f.at-(this.lastStart.get(f.symbol)??-Infinity)>=quantHorizon){
   const r:QuantSample={...f,features:{...f.features},id:`${f.symbol}:${f.connection}:${f.at}`,deadline:f.at+quantHorizon,outcome:'pending',resolvedAt:null,source:'live'};
   if(validQuantSample(r)){this.samples.push(r);this.lastStart.set(f.symbol,f.at);changed=true;reading.sampleId=r.id;}
  }else if(current)reading.sampleId=current.id;
  this.lastAt=f.at;
  if(changed){this.samples=this.samples.slice(-4000);if(this.persist)try{let saved=JSON.stringify(this.samples);while(saved.length>2000000&&this.samples.length>1){this.samples.shift();saved=JSON.stringify(this.samples);}localStorage.setItem(this.storageKey,saved);this.error='';}catch{this.error='Registro en memoria: almacenamiento no disponible';}}
  // Only an actually frozen forecast can publish a signal.
  if(!reading.sampleId&&reading.state==='signal'){reading.state='collecting';reading.side=0;reading.probabilities=null;reading.label='CUANTITATIVO · esperando nuevo escenario';}
  return reading;
 }
}
