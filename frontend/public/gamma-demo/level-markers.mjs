// Demo concentrations. Volume is activity, not resting order-book liquidity.
function barrierCandidates(rows,minStrength=50){
 const valid=rows.filter(r=>Number.isFinite(r.k)&&['score','volume','oiCall','oiPut','call','put'].every(key=>Number.isFinite(r[key])&&r[key]>=0));
 const max=fn=>Math.max(0,...valid.map(fn)),ratio=(v,m)=>m>0?v/m:0;
 const oi=max(r=>r.oiCall+r.oiPut),volume=max(r=>r.volume),gamma=max(r=>r.call+r.put);
 const candidates=valid.map(r=>({strike:r.k,importance:Math.min(100,r.score),strength:100*(.35*ratio(r.oiCall+r.oiPut,oi)+.2*ratio(r.volume,volume)+.3*ratio(r.call+r.put,gamma)+.15*Math.min(1,r.score/100))})).filter(r=>r.strength>=minStrength);
 return candidates;
}
export function liquidityBarriers(rows,spot){
 if(!Number.isFinite(spot)||spot<=0)return {upper:null,lower:null};
 const candidates=barrierCandidates(rows);
 return {upper:candidates.filter(r=>r.strike>spot).sort((a,b)=>a.strike-b.strike)[0]??null,lower:candidates.filter(r=>r.strike<spot).sort((a,b)=>b.strike-a.strike)[0]??null};
}
export function routeBarriers(rows,main,putWall,callWall){
 if(![main,putWall,callWall].every(v=>Number.isFinite(v)&&v>0))return {upper:[],lower:[]};
 const candidates=barrierCandidates(rows);
 return {upper:main<callWall?candidates.filter(r=>r.strike>main&&r.strike<callWall).sort((a,b)=>a.strike-b.strike):[],lower:putWall<main?candidates.filter(r=>r.strike<main&&r.strike>putWall).sort((a,b)=>b.strike-a.strike):[]};
}
export function directionalPulls(rows,spot,putWall,callWall){
 if(![spot,putWall,callWall].every(v=>Number.isFinite(v)&&v>0)||putWall>=callWall)return {call:null,put:null};
 const concentrations=barrierCandidates(rows,0);
 const put=concentrations.find(r=>r.strike===putWall),call=concentrations.find(r=>r.strike===callWall);
 const evaluate=(support,target)=>{
  if(!support||!target)return null;
  const between=concentrations.filter(r=>r.strike>Math.min(spot,target.strike)&&r.strike<Math.max(spot,target.strike));
  const open=between.length?100-between.reduce((s,r)=>s+r.strength,0)/between.length:0;
  const weakest=between.length?100-Math.min(...between.map(r=>r.strength)):0;
  // Snipers' pull weights, with illustrative demo support/path/vacuum inputs.
  return {target:target.strike,strength:.3*support.importance+.35*target.strength+.2*open+.15*weakest};
 };
 return {call:spot>=putWall&&spot<callWall?evaluate(put,call):null,put:spot<=callWall&&spot>putWall?evaluate(call,put):null};
}
export function levelMarkers(rows,mainMagnet) {
 const valid=rows.filter(r=>Number.isFinite(r.k)&&['score','volume','oiCall','oiPut','call','put'].every(key=>Number.isFinite(r[key])&&r[key]>=0));
 const maximum=fn=>Math.max(0,...valid.map(fn));
 const maxVolume=maximum(r=>r.volume),maxOi=maximum(r=>r.oiCall+r.oiPut),maxGamma=maximum(r=>r.call+r.put);
 const gammaValid=r=>[r.gammaCall,r.gammaPut].every(v=>Number.isFinite(v)&&v>=0)&&r.oiCall+r.oiPut>0;
 const averageGamma=r=>gammaValid(r)?(r.gammaCall*r.oiCall+r.gammaPut*r.oiPut)/(r.oiCall+r.oiPut):0;
 const maxNet=maximum(r=>Math.abs(r.call-r.put)),maxContractGamma=maximum(averageGamma);
 const maxGrowth=maximum(r=>Number.isFinite(r.gexChange)?Math.max(0,r.gexChange):0);
 const ratio=(v,max)=>max>0?v/max:0;
 const enriched=valid.map(r=>({...r,volumePct:ratio(r.volume,maxVolume)*100,concentration:100*(.35*ratio(r.oiCall+r.oiPut,maxOi)+.2*ratio(r.volume,maxVolume)+.3*ratio(r.call+r.put,maxGamma)+.15*Math.min(1,r.score/100))}));
 const pool=enriched.slice().sort((a,b)=>b.concentration-a.concentration||a.k-b.k)[0];
 const markers=[['put-wall','Put Wall',730],['liquidity','Imán de liquidez',pool?.k],['main','Imán principal',mainMagnet],['call-wall','Call Wall',745]];
 const result=markers.map(([id,label,strike])=>{
  const row=enriched.find(r=>r.k===strike);
  const gammaReady=row&&[row.gammaCall,row.gammaPut].every(v=>Number.isFinite(v)&&v>=0)&&(row.gammaCall*row.oiCall+row.gammaPut*row.oiPut)>0;
  const hasHistory=Number.isFinite(row?.gexChange);
  const gammaScore=gammaReady&&maxGamma>0?100*(.5*ratio(row.call+row.put,maxGamma)+.2*ratio(Math.abs(row.call-row.put),maxNet)+.2*ratio(averageGamma(row),maxContractGamma)+(hasHistory?.1*ratio(Math.max(0,row.gexChange),maxGrowth):0))/(hasHistory?1:.9):null;
  return {id,label,strike:row?.k??null,importance:row?Math.max(1,Math.min(100,row.score)):null,volume:row?.volumePct??null,concentration:row?.concentration??null,contracts:row?.volume??null,change:Number.isFinite(row?.delta)?row.delta:null,dominanceScore:gammaScore};
 });
 // At most two leaders. A second qualifies within two model points of the first.
 // Gamma-family concentration score: gross/net GEX, mean contract gamma,
 // and recent exposure growth. Correlated features, not probability votes.
 // Volume, premium, proximity and the mixed importance score do not vote here.
 const ranked=result.filter(r=>r.dominanceScore!==null&&r.dominanceScore>0).sort((a,b)=>b.dominanceScore-a.dominanceScore||a.strike-b.strike);
 const distinct=ranked.filter((r,i,a)=>a.findIndex(x=>x.strike===r.strike)===i);
 const leaders=new Set(distinct.filter(r=>r.dominanceScore>=distinct[0].dominanceScore-2).slice(0,2).map(r=>r.strike));
 return result.map(r=>({...r,dominant:r.dominanceScore!==null&&leaders.has(r.strike)}));
}
