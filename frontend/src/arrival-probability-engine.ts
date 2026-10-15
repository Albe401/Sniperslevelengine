/** Independent target-arrival probabilities. CALL/PUT are separate events, not complementary votes. */
export const arrivalVersion='target-arrival-close-v1';
export type ArrivalHorizon='10m'|'session'|'swing2';
export interface ArrivalFrame {symbol:string;side:1|-1;horizon:ArrivalHorizon;at:number;deadline:number;origin:number;target:number;features:Record<string,number|null>;ready:boolean}
export interface ArrivalSample extends ArrivalFrame {id:string;session:string;source:'live'|'synthetic';outcome:'hit'|'miss';resolvedAt:number}
export interface ArrivalModel {version:string;symbol:string;side:1|-1;horizon:ArrivalHorizon;source:'live'|'synthetic';columns:string[];means:number[];scales:number[];min:number[];max:number[];present:number[];weights:number[];intercept:number;temperature:number;trainedThrough:number;calibratedThrough:number;validatedThrough:number;counts:{train:number;calibration:number;test:number;sessions:number};brier:number;baselineBrier:number;logLoss:number;baselineLogLoss:number;bands:{bin:number;n:number;predicted:number;observed:number;low:number;high:number}[]}
const finite=(v:unknown):v is number=>typeof v==='number'&&Number.isFinite(v);
const day=(t:number)=>new Date(t).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
const mean=(v:number[])=>v.reduce((a,b)=>a+b,0)/v.length;
const sigmoid=(z:number)=>z>=0?1/(1+Math.exp(-z)):Math.exp(z)/(1+Math.exp(z));
export const validArrivalFrame=(f:ArrivalFrame)=>!!f&&f.ready&&typeof f.symbol==='string'&&[1,-1].includes(f.side)&&['10m','session','swing2'].includes(f.horizon)&&[f.at,f.deadline,f.origin,f.target].every(finite)&&f.at>0&&f.deadline>f.at&&f.deadline-f.at<=7*86400000&&f.origin>0&&f.target>0&&(f.target-f.origin)*f.side>0&&(f.horizon!=='10m'||f.deadline-f.at===600000)&&!!f.features&&Object.keys(f.features).length>0&&Object.values(f.features).every(v=>v===null||finite(v));
const validSample=(s:ArrivalSample)=>validArrivalFrame(s)&&typeof s.id==='string'&&s.session===day(s.at)&&['live','synthetic'].includes(s.source)&&['hit','miss'].includes(s.outcome)&&finite(s.resolvedAt)&&s.resolvedAt>s.at&&s.resolvedAt<=s.deadline&&(s.outcome!=='miss'||s.resolvedAt===s.deadline);
const encode=(f:Record<string,number|null>,m:ArrivalModel)=>m.columns.flatMap((k,i)=>finite(f[k])?[(f[k]!-m.means[i])/m.scales[i],0]:[0,1]);
const estimate=(f:Record<string,number|null>,m:ArrivalModel)=>{const x=encode(f,m),z=m.weights.reduce((sum,w,i)=>sum+w*x[i],m.intercept)/m.temperature;return finite(z)?sigmoid(z):NaN;};
/** Raw offline score only; production must use readArrivalProbability and its validation gates. */
export {estimate as arrivalEstimateForResearch,validSample as validArrivalSample};
const loss=(p:number,y:number)=>-y*Math.log(Math.max(1e-12,p))-(1-y)*Math.log(Math.max(1e-12,1-p));
const wilson=(hits:number,n:number)=>{const p=hits/n,z=1.96,z2=z*z,d=1+z2/n,c=(p+z2/(2*n))/d,h=z*Math.sqrt(p*(1-p)/n+z2/(4*n*n))/d;return {low:c-h,high:c+h};};
/** Offline only: purges overlapping observation windows, then splits by session 60/20/20. */
export function trainArrivalProbability(input:ArrivalSample[],symbol:string,side:1|-1,horizon:ArrivalHorizon){
 const rows:ArrivalSample[]=[];let end=-Infinity;const ids=new Set<string>();
 for(const row of input.filter(validSample).filter(s=>s.symbol===symbol&&s.side===side&&s.horizon===horizon).sort((a,b)=>a.at-b.at)){if(ids.has(row.id)||row.at<end)continue;rows.push(row);ids.add(row.id);end=row.deadline;}
 const sessions=[...new Set(rows.map(s=>s.session))].sort(),fail=(reason:string)=>({model:undefined as ArrivalModel|undefined,publishable:false,reason});
 if(sessions.length<30||new Set(rows.map(s=>s.source)).size!==1)return fail('Requiere 30 sesiones sin mezclar fuentes reales y sintéticas');
 const c=sessions[Math.floor(sessions.length*.6)],t=sessions[Math.floor(sessions.length*.8)],ca=rows.find(s=>s.session===c)!.at,ta=rows.find(s=>s.session===t)!.at;
 const train=rows.filter(s=>s.session<c&&s.deadline<ca),cal=rows.filter(s=>s.session>=c&&s.session<t&&s.deadline<ta),test=rows.filter(s=>s.session>=t);
 if(train.length<300||cal.length<100||test.length<100||[train,cal,test].some(part=>['hit','miss'].some(o=>part.filter(s=>s.outcome===o).length<10)))return fail('Muestra temporal insuficiente de llegadas y no llegadas (300/100/100)');
 const columns=Object.keys(train[0].features).sort();if(rows.some(s=>Object.keys(s.features).sort().join('|')!==columns.join('|')))return fail('Variables incompatibles');
 const values=columns.map(k=>train.map(s=>s.features[k]).filter(finite)),means=values.map(v=>v.length?mean(v):0);
 const m:ArrivalModel={version:arrivalVersion,symbol,side,horizon,source:rows[0].source,columns,means,scales:values.map((v,i)=>v.length?Math.max(1e-6,Math.sqrt(mean(v.map(x=>(x-means[i])**2)))):1),min:values.map(v=>v.length?Math.min(...v):0),max:values.map(v=>v.length?Math.max(...v):0),present:values.map(v=>v.length),weights:Array(columns.length*2).fill(0),intercept:0,temperature:1,trainedThrough:Math.max(...train.map(s=>s.deadline)),calibratedThrough:Math.max(...cal.map(s=>s.deadline)),validatedThrough:Math.max(...test.map(s=>s.deadline)),counts:{train:train.length,calibration:cal.length,test:test.length,sessions:sessions.length},brier:0,baselineBrier:0,logLoss:0,baselineLogLoss:0,bands:[]};
 const xs=train.map(s=>encode(s.features,m)),ys=train.map(s=>s.outcome==='hit'?1:0);
 for(let epoch=0;epoch<400;epoch++){const g=m.weights.map(()=>0);let bias=0;xs.forEach((x,j)=>{const error=sigmoid(m.weights.reduce((z,w,i)=>z+w*x[i],m.intercept))-ys[j];bias+=error;x.forEach((v,i)=>g[i]+=error*v);});m.intercept-=.05*bias/train.length;m.weights=m.weights.map((w,i)=>w-.05*(g[i]/train.length+.02*w));}
 let best=Infinity;for(const temperature of [.5,.75,1,1.5,2,3,5]){m.temperature=temperature;const score=mean(cal.map(s=>loss(estimate(s.features,m),s.outcome==='hit'?1:0)));if(score<best){best=score;(m as ArrivalModel&{bestTemperature?:number}).bestTemperature=temperature;}}
 m.temperature=(m as ArrivalModel&{bestTemperature?:number}).bestTemperature??1;delete (m as ArrivalModel&{bestTemperature?:number}).bestTemperature;
 const baseline=mean(ys),ps=test.map(s=>estimate(s.features,m)),y=test.map(s=>s.outcome==='hit'?1:0);
 if(ps.some(p=>!finite(p)))return fail('Desbordamiento matemático');
 m.brier=mean(ps.map((p,i)=>(p-y[i])**2));m.baselineBrier=mean(y.map(v=>(baseline-v)**2));m.logLoss=mean(ps.map((p,i)=>loss(p,y[i])));m.baselineLogLoss=mean(y.map(v=>loss(baseline,v)));
 for(let bin=0;bin<10;bin++){const indexes=ps.flatMap((p,i)=>Math.min(9,Math.floor(p*10))===bin?[i]:[]);if(!indexes.length)continue;const hits=indexes.reduce((z,i)=>z+y[i],0);m.bands.push({bin,n:indexes.length,predicted:mean(indexes.map(i=>ps[i])),observed:hits/indexes.length,...wilson(hits,indexes.length)});}
 return {model:m,publishable:m.source==='live'&&m.brier<m.baselineBrier&&m.logLoss<m.baselineLogLoss&&m.bands.some(b=>b.n>=100&&b.high-b.low<=.2&&Math.abs(b.predicted-b.observed)<=.1),reason:'Modelo de investigación; exige validación prospectiva adicional antes de publicación'};
}
/** Re-evaluates from current context; no manual increments, orders, or forced direction. */
export function readArrivalProbability(f:ArrivalFrame,m?:ArrivalModel){
 const pending=(reason:string)=>({probability:null as number|null,reason,horizon:f.horizon,target:f.target,at:f.at});
 if(!validArrivalFrame(f))return pending('Precio, objetivo, horizonte o variables no válidos');
 if(!m)return pending('Modelo probabilístico pendiente de entrenamiento y validación real');
 if(m.version!==arrivalVersion||m.source!=='live'||m.symbol!==f.symbol||m.side!==f.side||m.horizon!==f.horizon||![m.trainedThrough,m.calibratedThrough,m.validatedThrough,m.brier,m.baselineBrier,m.logLoss,m.baselineLogLoss,m.temperature,m.intercept].every(finite)||m.temperature<=0||m.brier<0||m.baselineBrier>1||m.logLoss<0||m.brier>=m.baselineBrier||m.logLoss>=m.baselineLogLoss||!(m.trainedThrough<m.calibratedThrough&&m.calibratedThrough<m.validatedThrough&&m.validatedThrough<f.at)||f.at-m.validatedThrough>30*86400000||!m.counts||Object.values(m.counts).some(v=>!Number.isInteger(v)||v<=0)||m.counts.train<300||m.counts.calibration<100||m.counts.test<100||m.counts.sessions<30)return pending('Modelo sin validación temporal compatible');
 if(!Array.isArray(m.columns)||!Array.isArray(m.bands)||!m.columns.length||m.columns.join('|')!==Object.keys(f.features).sort().join('|')||[m.means,m.scales,m.min,m.max,m.present].some(v=>!Array.isArray(v)||v.length!==m.columns.length)||!Array.isArray(m.weights)||m.weights.length!==2*m.columns.length||[...m.means,...m.scales,...m.min,...m.max,...m.present,...m.weights].some(v=>!finite(v))||m.scales.some(v=>v<=0)||m.min.some((v,i)=>v>m.max[i])||m.present.some(v=>!Number.isInteger(v)||v<0||v>m.counts.train))return pending('Parámetros o esquema inválidos');
 for(let i=0;i<m.columns.length;i++){const v=f.features[m.columns[i]];if(v===null&&m.present[i]===m.counts.train)return pending('Patrón de datos ausentes no observado en entrenamiento');const margin=Math.max(m.scales[i],.2*(m.max[i]-m.min[i]));if(finite(v)&&(m.present[i]===0||v<m.min[i]-margin||v>m.max[i]+margin))return pending('Escenario fuera de muestra');}
 const p=estimate(f.features,m);if(!finite(p)||p<0||p>1)return pending('Cálculo numérico inválido');
 const matchingBands=m.bands.filter(b=>b.bin===Math.min(9,Math.floor(p*10))),band=matchingBands.length===1?matchingBands[0]:undefined;
 if(!band||!Number.isInteger(band.n)||band.n<100||![band.predicted,band.observed,band.low,band.high].every(v=>finite(v)&&v>=0&&v<=1)||band.low>band.high||band.observed<band.low-1e-12||band.observed>band.high+1e-12||band.high-band.low>.2||Math.abs(band.predicted-band.observed)>.1)return pending('Este porcentaje no tiene banda calibrada suficiente');
 return {probability:p,reason:`Llegada por cierre antes del plazo · ${band.n} casos de validación en esta banda · experimental`,horizon:f.horizon,target:f.target,at:f.at};
}
