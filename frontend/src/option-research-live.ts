import {invoke,isTauri} from './snapshot-observation';
import type {BaselinePacket} from './option-baseline-archive';
let message='Motores de actividad: esperando conexión QQQ';
let busy=false,lastPoll=0,lastPacket=0;
export interface OptionResearchStatus {
 status:string;targetDate?:string;
 audit?:{generatedMs?:number;summary?:{bothComparablePriceCompatibleContractWindows?:number;certifiedPremiumContractWindows?:number;certifiedVolumeContractWindows?:number};reconciliation?:OptionActivityRow[]};
}
export interface OptionActivityRow {
 underlying:string;date:string;endMs:number;priceContextCompatible?:boolean;
 volume?:{comparable:boolean;unusual:boolean}|null;
 premium?:{comparable:boolean;unusual:boolean}|null;
}
let snapshot:OptionResearchStatus|undefined;
export function optionResearchSnapshot(){return snapshot;}
export function optionResearchNote(){return message;}
export function pollOptionResearch(now=Date.now()){
 if(typeof window==='undefined'||!isTauri()||busy||now-lastPoll<10000)return;busy=true;lastPoll=now;
 void invoke<OptionResearchStatus>('get_option_research_status').then(r=>{
  snapshot=r;
  const s=r.audit?.summary;
  message=s?`Motores QQQ · volumen ${s.certifiedVolumeContractWindows??0} · primas ${s.certifiedPremiumContractWindows??0} · comparables ${s.bothComparablePriceCompatibleContractWindows??0}`:`Motores QQQ: ${r.status}`;
 }).catch(()=>{snapshot=undefined;message='Motores QQQ: estado no disponible';}).finally(()=>{busy=false;});
}
export function persistOptionBaseline(packet:BaselinePacket){
 if(typeof window==='undefined'||!isTauri()||packet.symbol!=='QQQ'||Math.floor(packet.observedMs/1000)<=lastPacket)return;
 const second=Math.floor(packet.observedMs/1000);lastPacket=second;
 void invoke('record_option_formula_baseline',{payload:packet}).catch(()=>{if(lastPacket===second)lastPacket=second-1;message='Motores QQQ: no se pudo guardar el diario de la fórmula';});
}
