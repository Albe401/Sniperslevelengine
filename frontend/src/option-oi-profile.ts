import type {Heatmap,Side} from './option-oi-heatmap';
import type {EtfReport} from './etf-dealers';
import {reportOiHeatmap} from './option-oi-heatmap-report';

/** Projection, never a second OI calculation. Unknown cells invalidate their side's total. */
export function oiStrikeRows(map:Heatmap){
 const groups=new Map<number,{call:number|null;put:number|null}>();
 for(const strike of map.strikes)groups.set(strike,{call:0,put:0});
 for(const c of map.cells){
  const g=groups.get(c.strike)!;
  const value=c.state==='known'?c.oi:c.state==='excluded'||c.state==='no_contract'&&map.universeComplete?0:null;
  const side:Side=c.side;
  if(value===null||g[side]===null)g[side]=null;
  else {const sum=g[side]!+value;g[side]=Number.isSafeInteger(sum)?sum:null;}
 }
 return [...groups].map(([strike,r])=>({strike,...r}));
}
let cache:{report:EtfReport;map:Heatmap;rows:ReturnType<typeof oiStrikeRows>}|undefined;
export function reportOiProfile(report:EtfReport){
 if(cache?.report===report)return cache;
 const map=reportOiHeatmap(report,'matrix','linear');
 return cache={report,map,rows:oiStrikeRows(map)};
}
