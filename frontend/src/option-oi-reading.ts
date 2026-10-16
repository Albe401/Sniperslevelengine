import type {Heatmap,Side} from './option-oi-heatmap';

export interface OiSideReading {
 oi:number|null; complete:boolean; knownCells:number; unresolvedCells:number;
 knownSeries:number; expectedSeries:number; precisionError:boolean;
 topStrike:{strike:number;oi:number;ties:number}|null;
}
/** Same filtered, deduplicated cells as the heatmap. Counts contracts, never exposures or trades. */
export function readOiMap(map:Heatmap){
 const side=(name:Side):OiSideReading=>{
  let sum=0,knownCells=0,unresolvedCells=0,knownSeries=0,expectedSeries=0,precisionError=false;
  const strikes=new Map<number,number>();
  for(const cell of map.cells){
   if(cell.side!==name)continue;
   expectedSeries+=cell.expectedContracts;
   if(cell.state==='known'&&cell.oi!==null&&Number.isSafeInteger(cell.oi)&&cell.oi>=0){
    sum+=cell.oi;knownCells++;knownSeries+=cell.knownContracts;
    const value=(strikes.get(cell.strike)??0)+cell.oi;strikes.set(cell.strike,value);
    if(!Number.isSafeInteger(sum)||!Number.isSafeInteger(value))precisionError=true;
   }else if(cell.state!=='excluded'&&!(cell.state==='no_contract'&&map.universeComplete))unresolvedCells++;
  }
  const hasDomain=map.strikes.length>0&&map.expirations.length>0;
  const complete=hasDomain&&map.universeComplete&&unresolvedCells===0&&!precisionError;
  const oi=precisionError||!hasDomain||(!knownCells&&!complete)?null:sum;
  const ranked=[...strikes].filter(([,v])=>v>0).sort((a,b)=>b[1]-a[1]||a[0]-b[0]);
  const first=ranked[0];
  return {oi,complete,knownCells,unresolvedCells,knownSeries,expectedSeries,precisionError,
   topStrike:!precisionError&&first?{strike:first[0],oi:first[1],ties:ranked.filter(([,v])=>v===first[1]).length}:null};
 };
 const call=side('call'),put=side('put');
 const sum=call.oi!==null&&put.oi!==null?call.oi+put.oi:null;
 const total=call.complete&&put.complete&&sum!==null&&Number.isSafeInteger(sum)?sum:null;
 const comparable=total!==null&&total>0;
 return {call,put,total,totalPrecisionError:sum!==null&&!Number.isSafeInteger(sum),callPct:comparable?100*call.oi!/total:null,putPct:comparable?100*put.oi!/total:null,
  leader:comparable?(call.oi!>put.oi!?'call':put.oi!>call.oi!?'put':'equal'):null,
  difference:comparable?Math.abs(call.oi!-put.oi!):null};
}
