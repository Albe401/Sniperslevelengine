import {invoke,isTauri} from './snapshot-observation';
import {ivRank} from './iv-rank';
interface Observation {date:string;at:number;iv:number;ivLow?:number;ivHigh?:number;modelVersion?:number}
interface History {symbol:string;status:string;expected:number;start:string;end:string;observations:Observation[];running?:boolean;modelVersion?:number;experimentalProgress?:number}
const cache=new Map<string,History>(),requested=new Map<string,number>();
export function readIvHistory(h:History|undefined,symbol:string,now:number){
 const unique=new Map<string,Observation>();
 for(const o of h?.symbol===symbol?h.observations??[]:[]){if((h?.modelVersion!==2||o.modelVersion===2)&&Number.isFinite(o.iv)&&o.iv>0&&Number.isFinite(o.at)&&o.at<=now&&now-o.at<=364*86400000&&/^\d{4}-\d{2}-\d{2}$/.test(o.date))unique.set(o.date,o);}
 const rows=[...unique.values()].sort((a,b)=>a.at-b.at);
 const last=rows.at(-1);const full=!!h&&h.symbol===symbol&&h.expected>=240&&h.expected<=270&&rows.length>=240&&rows.length/h.expected>=.98&&rows.length<=h.expected&&!!rows[0]&&Date.parse(rows[0].date)-Date.parse(h.start)<=7*86400000&&!!last&&Date.parse(h.end)-Date.parse(last.date)<=4*86400000&&now-last.at<=7*86400000;
 const low=full?Math.min(...rows.map(o=>o.iv)):null,high=full?Math.max(...rows.map(o=>o.iv)):null;
 const bounded=full&&rows.every(o=>Number.isFinite(o.ivLow)&&Number.isFinite(o.ivHigh)&&o.ivLow!>0&&o.ivLow!<=o.iv&&o.ivHigh!>=o.iv);
 const lower=bounded?ivRank(last!.ivLow!,Math.min(...rows.map(o=>o.ivHigh!)),Math.max(...rows.map(o=>o.ivHigh!))).rank:null;
 const upper=bounded?ivRank(last!.ivHigh!,Math.min(...rows.map(o=>o.ivLow!)),Math.max(...rows.map(o=>o.ivLow!))).rank:null;
 return {...ivRank(full&&last?last.iv:null,low,high),current:last?.iv??null,low,high,date:last?.date??'',at:last?.at??0,progress:`${rows.length}/${h?.expected??'—'} sesiones`,full,modelVersion:h?.modelVersion??1,experimentalProgress:h?.experimentalProgress,ivBand:bounded?[last!.ivLow!,last!.ivHigh!]:null,rankBand:lower!=null&&upper!=null?[lower,upper]:null};
}
export function historicalIv(symbol:string,now:number){
 if(isTauri()&&now-(requested.get(symbol)??0)>15000){requested.set(symbol,now);void invoke<History>('get_iv_history',{symbol}).then(h=>{if(h.symbol===symbol)cache.set(symbol,h);}).catch(()=>{});}
 return readIvHistory(cache.get(symbol),symbol,now);
}
