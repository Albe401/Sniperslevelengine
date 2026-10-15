import {chartContractReport} from './contract-profile';
import type {Horizon,LiveState} from './live';
import {interpretationFresh} from './confirmations';
import {rangeFresh} from './daily-range';
import {displayStructure,currentMapHorizon} from './reading-display';
export interface GammaRow {strike:number;call_oi:number;put_oi:number;call_gex:number;put_gex:number;net_gex:number;gross_gex:number;total_oi:number;total_volume:number;total_gamma_weight:number;score:number}
export interface GammaLevel {id:string;label:string;strike:number|null;importance:number|null;force:number|null;liquidity:number|null;dominant:boolean}
export function gammaLiveModel(state:LiveState|undefined,symbol:string,active:boolean,now=Date.now()) {
 const h=currentMapHorizon(state?.optionSnapshotStatus,now);
 const fresh=!!(active&&state?.symbol===symbol&&state.optionSnapshotStatus.phase==='ready'&&rangeFresh(state.optionSnapshotStatus,now)&&h&&h.observedMs===state.optionSnapshotStatus.observedMs&&rangeFresh({observedMs:h.observedMs,phase:'ready'},now));
 // Full-universe comparisons require full Snipers structural and GEX coverage.
 const liveFresh=!!(state&&interpretationFresh(state.publishedMs,now));
 const complete=!!(fresh&&liveFresh&&h?.mode==='DAY'&&h.structure.complete&&h.structure.gexAvailable&&h.exposure.gexComplete);
 const supplied=fresh?(h?.structure.strikeRows??[]):[];
 const near=(a:number,b:number)=>Number.isFinite(b)&&Math.abs(a-b)<=Math.max(1,Math.abs(a),Math.abs(b))*1e-8;
 const valid=(r:GammaRow)=>Number.isFinite(r.strike)&&r.strike>0&&[r.call_oi,r.put_oi,r.call_gex,r.put_gex,r.net_gex,r.gross_gex,r.total_oi,r.total_volume,r.total_gamma_weight,r.score].every(Number.isFinite)&&[r.call_oi,r.put_oi,r.call_gex,r.put_gex,r.gross_gex,r.total_oi,r.total_volume,r.total_gamma_weight,r.score].every(v=>v>=0)&&r.score<=100&&(r.total_oi>0?Number.isFinite(r.total_gamma_weight/r.total_oi):r.total_gamma_weight===0)&&near(r.total_oi,r.call_oi+r.put_oi)&&near(r.gross_gex,r.call_gex+r.put_gex)&&near(r.net_gex,r.call_gex-r.put_gex);
 // Invalid or duplicate strikes invalidate comparative ranking, rather than bias it.
 const profileRows=supplied.every(valid)&&new Set(supplied.map(r=>r.strike)).size===supplied.length?supplied.slice().sort((a,b)=>a.strike-b.strike):[];
 const partial=h?.structure.provisional?.strikeRows??[];
 const gexRows=fresh&&partial.every(valid)&&new Set(partial.map(r=>r.strike)).size===partial.length?partial.slice().sort((a,b)=>a.strike-b.strike):[];
 const profileGexRows=fresh&&h?.structure.gexAvailable?profileRows:gexRows;
 const rows=complete?profileRows:[];
 const max=(fn:(r:GammaRow)=>number)=>Math.max(0,...rows.map(fn));
 const gross=max(r=>r.gross_gex),net=max(r=>Math.abs(r.net_gex)),gamma=max(r=>r.total_oi>0?r.total_gamma_weight/r.total_oi:0),oi=max(r=>r.total_oi),volume=max(r=>r.total_volume);
 const ratio=(v:number,m:number)=>m>0?v/m:0;
 const force=(r:GammaRow)=>gross>0&&gamma>0&&r.gross_gex>0&&r.total_gamma_weight>0&&r.total_oi>0?100*(.5*ratio(r.gross_gex,gross)+.2*ratio(Math.abs(r.net_gex),net)+.2*ratio(r.total_gamma_weight/r.total_oi,gamma))/.9:null;
 // A known reference can be drawn before full-universe strength is available.
 const s=displayStructure(h,fresh);
 const refs:[string,string,number|null|undefined][]=[['put','Put Wall',s?.putWall?.strike],['liquidity','Imán de liquidez',s?.magnet],['main','Imán principal',s?.selected.M1],['call','Call Wall',s?.callWall?.strike]];
 const levels:GammaLevel[]=refs.map(([id,label,strike])=>{const r=rows.find(r=>r.strike===strike);return {id,label,strike:strike!=null&&Number.isFinite(strike)&&strike>0?strike:null,importance:r?.score??null,force:r?force(r):null,liquidity:r?100*(.35*ratio(r.total_oi,oi)+.2*ratio(r.total_volume,volume)+.3*ratio(r.gross_gex,gross)+.15*Math.min(1,r.score/100)):null,dominant:false};});
 const ranked=levels.filter(l=>l.force!=null).sort((a,b)=>b.force!-a.force!||a.strike!-b.strike!);
 const distinct=ranked.filter((l,i,a)=>a.findIndex(x=>x.strike===l.strike)===i);
 const leaders=new Set(distinct.filter(l=>l.force!>=distinct[0].force!-2).slice(0,2).map(l=>l.strike));
 levels.forEach(l=>l.dominant=l.force!=null&&leaders.has(l.strike));
 const main=s?.selected.M1,call=s?.callWall?.strike,put=s?.putWall?.strike;
 const barriers=(upper:boolean)=>rows.filter(r=>main!=null&&(upper?call!=null&&r.strike>main&&r.strike<call:put!=null&&r.strike<main&&r.strike>put)&&r.score>=50).sort((a,b)=>upper?a.strike-b.strike:b.strike-a.strike);
 const contractProfile=fresh?chartContractReport(state,symbol,active,h,now):undefined;
 return {contractProfile,contractProfileExpected:state?.optionSnapshotStatus?.structuralEnrichment!=null,h,complete,fresh,rows,profileRows,profileGexRows,levels,upper:barriers(true),lower:barriers(false),call:s?.routes.find(r=>r.direction==='CALL'),put:s?.routes.find(r=>r.direction==='PUT'),flip:complete?h?.exposure.zeroGamma:null,status:!active?'SIN CONEXIÓN':!fresh?'ESPERANDO SNAPSHOT VIGENTE':!liveFresh?'PUBLICACIÓN RETRASADA · NIVELES DEL SNAPSHOT VIGENTE':h?.mode==='NEXT_SESSION'?'PRÓXIMO VENCIMIENTO · REFERENCIAS DEL SNAPSHOT':!complete?'COBERTURA INCOMPLETA · PUNTUACIONES PENDIENTES':rows.length?'SNAPSHOT SNIPERS VIGENTE':'ESPERANDO PERFIL POR STRIKE'};
}

/** Keep current structural references inside the automatic chart scale. */
export function gammaPriceRange(minValue:number,maxValue:number,model:ReturnType<typeof gammaLiveModel>|undefined) {
 const values=[minValue,maxValue,...(model?.levels.map(l=>l.strike)??[]),model?.flip,model?.call?.vacuumLow,model?.call?.vacuumHigh,model?.put?.vacuumLow,model?.put?.vacuumHigh].filter((n):n is number=>typeof n==='number'&&Number.isFinite(n)&&n>0);
 return {minValue:Math.min(...values),maxValue:Math.max(...values)};
}

/** Contract bars are independent of structural scores, volume and comparative rankings. */
export function gammaContractRows(model:ReturnType<typeof gammaLiveModel>|undefined,metric:'gex'|'oi'):{strike:number;call:number|null;put:number|null}[]{
 if(model?.contractProfileExpected)return model.contractProfile?.rows.map(r=>({strike:r.strike,call:metric==='gex'?r.call.gamma:r.call.oi,put:metric==='gex'?r.put.gamma:r.put.oi}))??[];
 return (metric==='gex'?model?.profileGexRows:model?.profileRows)?.map(r=>({strike:r.strike,call:metric==='gex'?r.call_gex:r.call_oi,put:metric==='gex'?r.put_gex:r.put_oi}))??[];
}
