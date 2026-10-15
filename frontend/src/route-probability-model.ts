import {probabilityEndpoint,probabilityVersion,validProbabilitySample,type ProbabilitySample} from './route-probability';
export interface ProbabilityModel {
 version:string;endpoint:string;symbol:string;trainedThrough:number;calibratedThrough:number;validatedThrough:number;
 columns:string[];means:number[];scales:number[];min:number[];max:number[];present:number[];weights:number[];intercept:number;calibration:{slope:number;intercept:number};
 counts:{train:number;calibration:number;test:number;sessions:number};brier:number;baselineBrier:number;
 bins:{direction:number;band:number;n:number;predicted:number;observed:number;lower:number;upper:number;usable:boolean}[];
}
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v);
const sigmoid=(z:number)=>1/(1+Math.exp(-Math.max(-30,Math.min(30,z))));
const mean=(v:number[])=>v.reduce((a,b)=>a+b,0)/v.length;
const success=(r:ProbabilitySample)=>r.outcome==='hit'?1:0;
const closed=(r:ProbabilitySample)=>['hit','invalidated','timeout'].includes(r.outcome)&&finite(r.resolvedAt)&&r.resolvedAt!>r.at&&r.resolvedAt!<=r.deadline;
function encode(r:ProbabilitySample,model:Pick<ProbabilityModel,'columns'|'means'|'scales'>){return model.columns.flatMap((k,i)=>{const v=r.features[k];return finite(v)?[(v-model.means[i])/model.scales[i],0]:[0,1];});}
function logit(r:ProbabilitySample,m:ProbabilityModel){return encode(r,m).reduce((z,v,i)=>z+v*m.weights[i],m.intercept);}
function estimate(r:ProbabilitySample,m:ProbabilityModel){return sigmoid(m.calibration.slope*logit(r,m)+m.calibration.intercept);}
function wilson(wins:number,n:number){const p=wins/n,z=1.96,z2=z*z,den=1+z2/n,c=(p+z2/(2*n))/den,h=z*Math.sqrt(p*(1-p)/n+z2/(4*n*n))/den;return {lower:c-h,upper:c+h};}
/** Offline only. Three chronological session blocks; outcome windows are purged at boundaries. */
export function trainRouteProbability(input:unknown[],symbol:string):{model?:ProbabilityModel;reason:string;counts:Record<string,number>}{
 const candidates=input.filter(validProbabilitySample).filter(r=>r.symbol===symbol&&closed(r)).sort((a,b)=>a.at-b.at);
 const rows:ProbabilitySample[]=[];const ids=new Set<string>();let last=-Infinity;
 for(const row of candidates){if(ids.has(row.id)||row.at-last<600000)continue;ids.add(row.id);rows.push(row);last=row.at;}
 const sessions=[...new Set(rows.map(r=>r.session))].sort(),counts={samples:rows.length,sessions:sessions.length};
 if(sessions.length<20)return {reason:'Faltan al menos 20 sesiones reales con resultados de la referencia a 10 min',counts};
 const calDay=sessions[Math.floor(sessions.length*.6)],testDay=sessions[Math.floor(sessions.length*.8)];
 const calAt=rows.find(r=>r.session===calDay)!.at,testAt=rows.find(r=>r.session===testDay)!.at;
 const train=rows.filter(r=>r.session<calDay&&r.deadline<calAt),cal=rows.filter(r=>r.session>=calDay&&r.session<testDay&&r.deadline<testAt),test=rows.filter(r=>r.session>=testDay);
 Object.assign(counts,{train:train.length,calibration:cal.length,test:test.length});
 if(train.length<300||cal.length<100||test.length<100)return {reason:'Muestra insuficiente en entrenamiento, calibración o prueba futura (300/100/100)',counts};
 if([train,cal,test].some(part=>[1,-1].some(d=>part.filter(r=>r.direction===d&&success(r)===1).length<10||part.filter(r=>r.direction===d&&success(r)===0).length<10)))return {reason:'Faltan éxitos y fallos observados en ambos lados y en cada bloque temporal',counts};
 const columns=Object.keys(train[0].features).sort();
 if(rows.some(r=>Object.keys(r.features).sort().join('|')!==columns.join('|')))return {reason:'Esquemas de entrada incompatibles; no mezclar versiones',counts};
 const values=columns.map(k=>train.map(r=>r.features[k]).filter(finite));
 const means=values.map(v=>v.length?mean(v):0),scales=values.map((v,i)=>v.length?Math.max(1e-6,Math.sqrt(mean(v.map(x=>(x-means[i])**2)))):1);
 const m:ProbabilityModel={version:probabilityVersion,endpoint:probabilityEndpoint,symbol,trainedThrough:Math.max(...train.map(r=>r.deadline)),calibratedThrough:Math.max(...cal.map(r=>r.deadline)),validatedThrough:Math.max(...test.map(r=>r.deadline)),columns,means,scales,min:values.map(v=>v.length?Math.min(...v):0),max:values.map(v=>v.length?Math.max(...v):0),present:values.map(v=>v.length),weights:Array(columns.length*2).fill(0),intercept:0,calibration:{slope:1,intercept:0},counts:{train:train.length,calibration:cal.length,test:test.length,sessions:sessions.length},brier:0,baselineBrier:0,bins:[]};
 const xs=train.map(r=>encode(r,m)),ys=train.map(success);
 for(let epoch=0;epoch<300;epoch++){
  const gradient=Array(m.weights.length).fill(0);let bias=0;
  xs.forEach((x,j)=>{const error=sigmoid(x.reduce((z,v,i)=>z+v*m.weights[i],m.intercept))-ys[j];bias+=error;for(let i=0;i<x.length;i++)gradient[i]+=error*x[i];});
  for(let i=0;i<m.weights.length;i++)m.weights[i]-=.05*(gradient[i]/train.length+.01*m.weights[i]);m.intercept-=.05*bias/train.length;
 }
 const logits=cal.map(r=>logit(r,m));
 for(let epoch=0;epoch<300;epoch++){let a=0,b=0;logits.forEach((z,i)=>{const error=sigmoid(m.calibration.slope*z+m.calibration.intercept)-success(cal[i]);a+=error*z;b+=error;});m.calibration.slope=Math.max(0,m.calibration.slope-.02*a/cal.length);m.calibration.intercept-=.02*b/cal.length;}
 const predictions=test.map(r=>estimate(r,m)),baseline=(direction:number)=>mean(train.filter(r=>r.direction===direction).map(success));
 m.brier=mean(predictions.map((p,i)=>(p-success(test[i]))**2));m.baselineBrier=mean(test.map(r=>(baseline(r.direction)-success(r))**2));
 for(const direction of [1,-1])for(let band=0;band<5;band++){
  const indexes=test.map((r,i)=>({r,i})).filter(({r,i})=>r.direction===direction&&Math.min(4,Math.floor(predictions[i]*5))===band).map(x=>x.i);
  const n=indexes.length;if(!n)continue;const wins=indexes.reduce((v,i)=>v+success(test[i]),0),predicted=mean(indexes.map(i=>predictions[i])),observed=wins/n,interval=wilson(wins,n);
  m.bins.push({direction,band,n,predicted,observed,...interval,usable:n>=100&&interval.upper-interval.lower<=.2&&Math.abs(predicted-observed)<=.1&&m.brier<m.baselineBrier});
 }
 if(!m.bins.some(b=>b.usable))return {reason:'Prueba futura sin banda de probabilidad suficientemente respaldada o sin mejora frente a tasa base',counts};
 return {model:m,reason:'Modelo ajustado y calibrado con bloques temporales separados; bandas admitidas limitadas a la prueba futura',counts};
}
/** Fail closed for foreign, old, extrapolated or statistically unsupported predictions. */
export function predictRouteProbability(sample:ProbabilitySample,model:ProbabilityModel|undefined,now:number){
 const pending=(reason:string)=>({percent:null as number|null,lower:null as number|null,upper:null as number|null,n:0,reason});
 if(!model)return pending('Sin modelo calibrado para este activo y resultado a 10 min');
 const m=model;
 if(!m.calibration||!m.counts||Object.values(m.counts).some(v=>!Number.isInteger(v)||v<=0)||![m.columns,m.weights,m.bins,m.means,m.scales,m.min,m.max,m.present].every(Array.isArray))return pending('Modelo inválido');
 if(!validProbabilitySample(sample)||!finite(now)||sample.at>now||now-sample.at>3000||m.version!==probabilityVersion||m.endpoint!==probabilityEndpoint||m.symbol!==sample.symbol||m.validatedThrough>=sample.at||now-m.validatedThrough>30*86400000||!(m.trainedThrough<m.calibratedThrough&&m.calibratedThrough<m.validatedThrough)||m.brier>=m.baselineBrier)return pending('Modelo incompatible, caducado o sin validación temporal');
 if(!m.counts||m.counts.train<300||m.counts.calibration<100||m.counts.test<100||m.counts.sessions<20||m.columns.length===0||m.columns.slice().sort().join('|')!==Object.keys(sample.features).sort().join('|')||m.weights.length!==m.columns.length*2||[m.means,m.scales,m.min,m.max,m.present].some(a=>!a||a.length!==m.columns.length)||[m.intercept,m.calibration.slope,m.calibration.intercept,m.brier,m.baselineBrier,...m.means,...m.scales,...m.min,...m.max,...m.weights].some(v=>!finite(v))||m.scales.some(v=>v<=0))return pending('Modelo inválido');
 for(let i=0;i<m.columns.length;i++){const v=sample.features[m.columns[i]],margin=Math.max(m.scales[i],(m.max[i]-m.min[i])*.2);if(finite(v)&&(m.present[i]===0||v<m.min[i]-margin||v>m.max[i]+margin))return pending('Escenario fuera del rango de entrenamiento');}
 const p=estimate(sample,m),bin=m.bins.find(b=>b.direction===sample.direction&&b.band===Math.min(4,Math.floor(p*5)));
 if(!bin||!bin.usable||bin.n<100||![bin.lower,bin.upper,bin.predicted,bin.observed].every(v=>finite(v)&&v>=0&&v<=1)||bin.upper-bin.lower>.2||Math.abs(bin.predicted-bin.observed)>.1)return pending('Banda sin respaldo suficiente en la prueba futura');
 return {percent:Math.max(.1,Math.min(99.9,Math.round(p*1000)/10)),lower:Math.round(bin.lower*100),upper:Math.round(bin.upper*100),n:bin.n,reason:'Probabilidad estimada de cierre en la siguiente referencia antes de invalidarse, dentro de 10 min. Redondeo a 0,1 puntos, extremos limitados a 0,1–99,9%; no certeza. Intervalo: frecuencia de la banda en la prueba futura.'};
}
