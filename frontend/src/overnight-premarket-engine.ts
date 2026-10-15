import type {Side} from './overnight-types';
import type {CloseRead} from './overnight-close-engine';
import {etHour,etMinute} from './overnight-types';
export interface PremarketEvidence {now:number;date:string;price:number|null;priceMs:number|null;previousClose:number|null;gammaRegime:'POSITIVE'|'NEGATIVE'|null;netGex:number|null;zeroGamma:number|null;callWall:number|null;putWall:number|null;mainMagnet:number|null;barrier:number|null;vacuum:'UP'|'DOWN'|null;flow:Side;rvol:number|null;premarketBlocks?:Side}
export interface PremarketRead {state:'PENDIENTE'|'CONFIRMADO'|'INVALIDADO'|'MIXTO';gapPct:number|null;checks:string[];missing:string[];reason:string}
export function readPremarket(close:CloseRead,input:PremarketEvidence):PremarketRead{
 const hour=etHour(input.now),minute=etMinute(input.now);
 if(hour<4||hour>9||hour===9&&minute>=30)return {state:'PENDIENTE',gapPct:null,checks:[],missing:['ventana premarket 04:00–09:30 ET'],reason:'La confirmación se evalúa solo en premarket'};
 const missing:string[]=[];if(input.price==null||input.priceMs==null||input.price<=0||input.previousClose==null||input.previousClose<=0)missing.push('precio premarket y cierre anterior');
 if(input.gammaRegime==null||input.netGex==null||input.zeroGamma==null)missing.push('gamma/GEX');
 if(input.callWall==null||input.putWall==null||input.mainMagnet==null)missing.push('muros e imán');
 if(input.flow==='PENDING')missing.push('flujo actualizado');
 if(input.rvol==null)missing.push('RVOL');
 if(close.bias==='PENDING'||close.bias==='NEUTRAL')return {state:'PENDIENTE',gapPct:null,checks:[],missing,reason:'El cierre anterior no dejó un sesgo direccional confirmado'};
 if(missing[0]==='precio premarket y cierre anterior'||input.priceMs!>input.now||input.now-input.priceMs!>15000||input.date!==new Date(input.now).toLocaleDateString('sv-SE',{timeZone:'America/New_York'}))return {state:'PENDIENTE',gapPct:null,checks:[],missing,reason:'Precio premarket reciente pendiente'};
 const gapPct=100*(input.price!/input.previousClose!-1),up=close.bias==='BUY',checks:string[]=[];
 checks.push((up?gapPct>0:gapPct<0)?'Gap acompaña':'Gap contradice');
 if(input.premarketBlocks==='BUY'||input.premarketBlocks==='SELL')checks.push(input.premarketBlocks===close.bias?'Bloques premarket acompañan':'Bloques premarket contradicen');
 if(input.flow!=='PENDING'&&input.flow!=='NEUTRAL')checks.push(input.flow===close.bias?'Flujo acompaña':'Flujo contradice');
 if(input.rvol!=null&&Number.isFinite(input.rvol))checks.push(input.rvol>=1?'RVOL activo':'RVOL bajo');
 if(input.zeroGamma!=null&&input.gammaRegime)checks.push(`${input.gammaRegime==='POSITIVE'?'Gamma positiva':'Gamma negativa'} · ${input.price!>=input.zeroGamma?'sobre':'bajo'} Zero Gamma`);
 if(input.callWall!=null&&input.putWall!=null)checks.push(`Entre muros: ${input.price!>input.putWall&&input.price!<input.callWall?'sí':'no'}`);
 if(input.mainMagnet!=null)checks.push(`Imán ${input.mainMagnet.toFixed(2)}`);
 if(input.barrier!=null)checks.push(`Barrera ${input.barrier.toFixed(2)}`);
 if(input.vacuum)checks.push(`Vacío ${input.vacuum}`);
 const flowAligned=input.flow===close.bias,flowOpposed=input.flow!==close.bias&&input.flow!=='NEUTRAL'&&input.flow!=='PENDING';
 const blocksOpposed=input.premarketBlocks==='BUY'||input.premarketBlocks==='SELL'?input.premarketBlocks!==close.bias:false;
 const priceAligned=up?gapPct>=.1:gapPct<=-.1,priceOpposed=up?gapPct<=-.1:gapPct>=.1;
 const enough=missing.length===0;
 const state=!enough?'PENDIENTE':priceAligned&&flowAligned&&!blocksOpposed?'CONFIRMADO':priceOpposed&&(flowOpposed||blocksOpposed)?'INVALIDADO':'MIXTO';
 return {state,gapPct,checks,missing,reason:!enough?'Confluencia incompleta; no se certifica el escenario':state==='CONFIRMADO'?'Gap y flujo premarket apoyan la hipótesis de cierre':state==='INVALIDADO'?'Gap y flujo premarket contradicen la hipótesis de cierre':'Evidencia premarket mixta'};
}
