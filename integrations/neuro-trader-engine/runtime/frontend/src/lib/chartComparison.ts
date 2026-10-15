import {CONV_SIGNS,type LivePayload,type Mode,type Convention} from './types';
import type {Candle} from './candles';
export function chartField(p:LivePayload,mode:Mode,zdte:boolean,conv:Convention){
  const profile=p.profile;if(!profile)throw Error('Snapshot sin perfil');
  const grid=profile.grids[mode];if(!grid)throw Error(`Sin capa ${mode}`);
  const c=grid[zdte?'c_all':'c_ex0'],put=grid[zdte?'p_all':'p_ex0'],sign=CONV_SIGNS[conv];
  const rows=profile.prices.length,cols=profile.minutes.length;
  if(c.q.length!==rows||put.q.length!==rows||c.q.some(r=>r.length!==cols)||put.q.some(r=>r.length!==cols))throw Error('Dimensiones inconsistentes');
  const values=c.q.map((row,r)=>row.map((v,j)=>sign.c*v*c.unit+sign.p*put.q[r][j]*put.unit));
  if(values.flat().some(v=>!Number.isFinite(v)))throw Error('Campo no finito');
  const sorted=values.flat().map(Math.abs).sort((a,b)=>a-b);
  return {prices:profile.prices,minutes:profile.minutes,values,scale:sorted[Math.floor((sorted.length-1)*.995)]||1};
}
// Real NY session date; UTC conversion is only for chart coordinates, not a new price source.
export function sessionStart(date:string){
  const base=Date.parse(`${date}T09:30:00Z`);
  const local=new Intl.DateTimeFormat('en-US',{timeZone:'America/New_York',hour:'2-digit',hourCycle:'h23'}).format(new Date(base));
  return (base+(9-Number(local))*3600000)/1000;
}
export function chartCandles(rows:Candle[],start:number){
  return rows.map(b=>({time:start+b.minute*60,open:b.o,high:b.h,low:b.l,close:b.c}));
}
