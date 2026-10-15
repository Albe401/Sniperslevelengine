import type {SavedBlocks,Holdings,Side} from './overnight-types';
import {readBlockPressure,type BlockPressure} from './overnight-block-engine';
export interface ComponentPressure {symbol:string;weight:number;buy:number;sell:number;dominance:Side;coverage:number}
export interface QqqPressure {asOf:string;coveredWeight:number;weightedBalance:number|null;dominance:Side;components:ComponentPressure[];reason:string}
export function readQqqComponents(saved:SavedBlocks|null,holdings:Holdings|null,date:string):QqqPressure{
 const pending:QqqPressure={asOf:holdings?.asOf??'',coveredWeight:0,weightedBalance:null,dominance:'PENDING',components:[],reason:'Pesos oficiales o bloques de componentes pendientes'};
 if(!saved||saved.report.date!==date||!holdings||!holdings.rows.length||holdings.asOf>date||new Set(holdings.rows.map(r=>r.symbol)).size!==holdings.rows.length)return pending;
 let weighted=0,covered=0;const components:ComponentPressure[]=[];
 for(const h of holdings.rows){if(!Number.isFinite(h.weight)||h.weight<=0||h.weight>100)continue;const b:BlockPressure=readBlockPressure(saved,h.symbol,date);
  components.push({symbol:h.symbol,weight:h.weight,buy:b.buy,sell:b.sell,dominance:b.dominance,coverage:b.coverage});
  if(b.dominance==='PENDING'||b.buyPct==null)continue;
  covered+=h.weight;weighted+=h.weight*(2*b.buyPct/100-1);
 }
 const balance=covered?weighted/covered:null;
 return {asOf:holdings.asOf,coveredWeight:covered,weightedBalance:balance,components,dominance:covered<30||balance==null?'PENDING':balance>=.1?'BUY':balance<=-.1?'SELL':'NEUTRAL',reason:covered<30?'Menos de 30% del peso de QQQ con bloques clasificados':'Balance ponderado por peso de Invesco entre componentes cubiertos; no es flujo neto del ETF'};
}
