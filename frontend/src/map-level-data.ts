import type {LiveState} from './live';
export interface MapLevelRead {levels:{price:number;name:string}[];partial:boolean;historical:boolean;observedMs:number|null;covered:number|null;total:number|null;detail:string}
const positive=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n)&&n>0;
export function mapLevelData(state:LiveState|undefined,symbol:string,now:number):MapLevelRead {
 const empty:MapLevelRead={levels:[],partial:false,historical:true,observedMs:null,covered:null,total:null,detail:'Esperando una observación estructural identificada del activo.'};
 if(!state||state.symbol!==symbol)return empty;
 const status=state.optionSnapshotStatus,h=status?.horizons?.find(h=>h.mode==='DAY')??status?.horizons?.find(h=>h.mode==='NEXT_SESSION');
 if(!h||!Number.isFinite(h.observedMs)||h.observedMs<=0||h.observedMs>now||!h.structure)return empty;
 const s=h.structure,p=s.provisional;
 const primary=[{price:s.callWall?.strike,name:'Call Wall'},{price:s.putWall?.strike,name:'Put Wall'},{price:s.magnet,name:'Main Magnet'}];
 const hasPrimary=s.complete&&s.liquidityComplete!==false&&primary.every(l=>positive(l.price));
 const validSubset=p&&Number.isInteger(p.coveredContracts)&&Number.isInteger(p.totalContracts)&&p.coveredContracts>0&&p.coveredContracts<p.totalContracts;
 const candidates=hasPrimary?primary:validSubset?[{price:p.callWall?.strike,name:'Call Wall · parcial'},{price:p.putWall?.strike,name:'Put Wall · parcial'},{price:p.magnet,name:'Main Magnet · parcial'}]:Object.entries(s.selected??{}).map(([name,price])=>({price,name:`${name} · estructural parcial`}));
 const historical=now-h.observedMs>300000||new Date(h.observedMs).toLocaleDateString('en-CA',{timeZone:'America/New_York'})!==new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
 const levels=candidates.filter((l):l is {price:number;name:string}=>positive(l.price)).map(l=>({...l,name:l.name+(historical?' · histórico':'')}));
 const partial=!hasPrimary,coverage=validSubset?`${p.coveredContracts}/${p.totalContracts} contratos incluidos; ${p.totalContracts-p.coveredContracts} excluidos por datos requeridos faltantes.`:'';
 const age=historical?'Referencia histórica; no nivel vigente.':partial?'Referencia parcial; no representa la cadena completa.':'Niveles del modelo; no órdenes pendientes.';
 const phase=status.refreshStage??status.phase;
 return {levels,partial,historical,observedMs:h.observedMs,covered:validSubset?p.coveredContracts:null,total:validSubset?p.totalContracts:null,detail:[age,coverage,`Actualización: ${phase}.`,status.refreshError??'',!hasPrimary?'Gamma/cobertura incompleta: los niveles definitivos permanecen pendientes.':''].filter(Boolean).join(' ')};
}
