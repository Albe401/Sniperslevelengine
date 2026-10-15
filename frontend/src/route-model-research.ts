/** Offline research only. Never imported by the live chart or execution path. */
import {trainQuantRoute,quantEstimate,readQuantModel,quantClasses,validQuantSample,type QuantSample} from './quant-route-engine';
import {trainArrivalProbability,arrivalEstimateForResearch,readArrivalProbability,validArrivalSample,type ArrivalSample,type ArrivalHorizon} from './arrival-probability-engine';
type Row=QuantSample|ArrivalSample;
export function researchWindows<T extends {at:number;deadline:number;session:string}>(rows:T[],initial=60,size=20){
 if(!Number.isInteger(initial)||initial<30||!Number.isInteger(size)||size<1)throw new Error('Ventanas inválidas');
 const ordered=rows.slice().sort((a,b)=>a.at-b.at),sessions=[...new Set(ordered.map(r=>r.session))].sort();
 return Array.from({length:3},(_,i)=>{
  const cut=initial+i*size,end=cut+size;if(end>sessions.length)return undefined;
  const selected=new Set(sessions.slice(cut,end)),future=ordered.filter(r=>selected.has(r.session)),first=future[0].at;
  const training=ordered.filter(r=>r.session<sessions[cut]&&r.deadline<first);
  return {training,future,first,last:future.at(-1)!.deadline};
 }).filter((v):v is NonNullable<typeof v>=>v!=null);
}
export function researchMetrics(probabilities:number[][],labels:number[],baseline:number[]){
 if(!labels.length||probabilities.length!==labels.length||!baseline.length||baseline.some(v=>!Number.isFinite(v)||v<0||v>1)||Math.abs(baseline.reduce((a,b)=>a+b,0)-1)>1e-9||probabilities.some((p,i)=>p.length!==baseline.length||p.some(v=>!Number.isFinite(v)||v<0||v>1)||Math.abs(p.reduce((a,b)=>a+b,0)-1)>1e-9||!Number.isInteger(labels[i])||labels[i]<0||labels[i]>=baseline.length))throw new Error('Distribuciones o resultados inválidos');
 const avg=(v:number[])=>v.reduce((a,b)=>a+b,0)/v.length;
 // Binary Brier uses one event; multiclass uses the sum over the three classes.
 const score=(ps:number[][])=>({brier:avg(ps.map((p,i)=>p.reduce((z,v,k)=>z+(v-(labels[i]===k?1:0))**2,0)/(baseline.length===2?2:1))),logLoss:avg(ps.map((p,i)=>-Math.log(Math.max(1e-12,p[labels[i]]))))});
 const metrics=score(probabilities),base=score(probabilities.map(()=>baseline));
 const bands=Array.from({length:baseline.length},(_,label)=>Array.from({length:10},(_,bin)=>{
  const indices=probabilities.flatMap((p,i)=>Math.min(9,Math.floor(p[label]*10))===bin?[i]:[]);
  return indices.length?{label,bin,n:indices.length,predicted:avg(indices.map(i=>probabilities[i][label])),observed:avg(indices.map(i=>labels[i]===label?1:0))}:undefined;
 }).filter(v=>v!=null)).flat();
 return {...metrics,baselineBrier:base.brier,baselineLogLoss:base.logLoss,improvesBoth:metrics.brier<base.brier&&metrics.logLoss<base.logLoss,bands};
}
const variants=[{name:'full',remove:/$^/},{name:'withoutFlow',remove:/^(flow|cvd|stockPressure|stockCoverage|optionPressure|optionCoverage|blockPressure)/},{name:'withoutEMA',remove:/^ema/},{name:'withoutStructure',remove:/(strength|change|oiShare|gammaShare|volumeOi|vanna|charm|DeltaShare|fourHour|barriersAhead|structureSignal)/i}];
export function investigateRouteModels(input:{quantSamples?:unknown[];arrivalSamples?:unknown[]},symbol:string,kind:'quant'|'arrival',side:1|-1=1,horizon:ArrivalHorizon='10m'){
 const raw=kind==='quant'?(input.quantSamples??[]):(input.arrivalSamples??[]);
 const valid:Row[]=kind==='quant'?raw.filter(validQuantSample).filter(r=>r.symbol===symbol&&quantClasses.includes(r.outcome as typeof quantClasses[number])):raw.filter((r):r is ArrivalSample=>validArrivalSample(r as ArrivalSample)).filter(r=>r.symbol===symbol&&r.side===side&&r.horizon===horizon);
 const sources=[...new Set(valid.map(r=>r.source))],rows:Row[]=[];let end=-Infinity;const ids=new Set<string>();
 for(const r of valid.sort((a,b)=>a.at-b.at)){if(ids.has(r.id)||r.at<end)continue;ids.add(r.id);end=r.deadline;rows.push(r);}
 const base={kind,symbol,side:kind==='arrival'?side:null,horizon:kind==='arrival'?horizon:'10m',source:sources.length===1?sources[0]:'unknown',rows:rows.length,automaticPublication:false,executionEnabled:false,marketImprovementProven:false};
 if(sources.length!==1)return {...base,reason:'Faltan datos compatibles o hay fuentes mezcladas',experiments:[]};
 const windows=researchWindows(rows);if(windows.length<3)return {...base,reason:'Requiere al menos 120 sesiones para tres bloques futuros de 20 sesiones; además aplica mínimos de muestra del entrenador',experiments:[]};
 const experiments=variants.flatMap(variant=>{
  if(variant.name!=='full'&&!Object.keys(rows[0].features).some(k=>variant.remove.test(k)))return [];
  const folds=windows.map(w=>{
   const transform=(r:Row)=>({...r,features:Object.fromEntries(Object.entries(r.features).filter(([k])=>!variant.remove.test(k)))});
   const training=w.training.map(transform),future=w.future.map(transform),trained=kind==='quant'?trainQuantRoute(training,symbol):trainArrivalProbability(training as ArrivalSample[],symbol,side,horizon);
   const model=trained.model;if(!model)return {first:w.first,last:w.last,n:future.length,reason:trained.reason,metrics:undefined};
   if(model.validatedThrough>=w.first)throw new Error('Fuga temporal: validación cruza bloque futuro');
   // Baseline uses exactly the trainer's earliest 60% of prefix sessions, not future labels.
   const sessions=[...new Set(training.map(r=>r.session))].sort(),cut=sessions[Math.floor(sessions.length*.6)],calAt=training.find(r=>r.session===cut)!.at;
   const fit=training.filter(r=>r.session<cut&&r.deadline<calAt);
   const outcome=(r:Row)=>kind==='quant'?quantClasses.indexOf(r.outcome as typeof quantClasses[number]):r.outcome==='hit'?1:0;
   const classes=kind==='quant'?3:2,baseline=Array.from({length:classes},(_,k)=>fit.filter(r=>outcome(r)===k).length/fit.length);
   const ps=future.map(r=>{if(kind==='quant')return quantEstimate(r.features,model as Parameters<typeof quantEstimate>[1]);const p=arrivalEstimateForResearch(r.features,model as Parameters<typeof arrivalEstimateForResearch>[1]);return [1-p,p];});
   const available=future.filter(r=>kind==='quant'?readQuantModel(r as QuantSample,model as Parameters<typeof readQuantModel>[1]).probabilities!=null:readArrivalProbability(r as ArrivalSample,model as Parameters<typeof readArrivalProbability>[1]).probability!=null).length;
   return {first:w.first,last:w.last,n:future.length,trainedThrough:model.trainedThrough,calibratedThrough:model.calibratedThrough,validatedThrough:model.validatedThrough,productionEligible:available,productionCoverage:available/future.length,metrics:researchMetrics(ps,future.map(outcome),baseline)};
  });
  return [{variant:variant.name,folds,allFutureBlocksImprove:folds.length===3&&folds.every(f=>f.metrics?.improvesBoth)}];
 });
 const full=experiments.find(e=>e.variant==='full');
 const comparisons=experiments.map(e=>({...e,versusFull:e.variant==='full'?[]:e.folds.map((f,i)=>f.metrics&&full?.folds[i].metrics?{first:f.first,brierIncrease:f.metrics.brier-full.folds[i].metrics!.brier,logLossIncrease:f.metrics.logLoss-full.folds[i].metrics!.logLoss}:undefined).filter(v=>v!=null)}));
 return {...base,reason:'Métricas de scores crudos fuera de muestra, separadas de cobertura con los filtros de producción. Diferencias positivas versusFull indican deterioro al retirar variables. Ganancias pequeñas no implican significación estadística; requiere bootstrap por sesión y otro período prospectivo intacto. No selecciona ni publica el ganador.',trialCount:experiments.length*3,experiments:comparisons};
}
