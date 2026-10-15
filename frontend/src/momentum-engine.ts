export interface MomentumInput {
 symbol:string;session:string;at:number;price:number;baseline:string;direction:number;
 flow:number|null;volumeRatio:number|null;priceAligned:boolean;routeConfirmed:boolean;
 target:number|null;stop:number|null;atr:number|null;ready:boolean;reason:string;
}
export interface MomentumRead {side:string;state:string;score:number|null;acceleration:number|null;space:number|null;reason:string}
const pending=(reason:string):MomentumRead=>({side:'PENDIENTE',state:'NO CONFIRMA',score:null,acceleration:null,space:null,reason});
/** Shadow model v1. Fixed heuristic weights, never a probability or an order trigger. */
export class MomentumEngine {
 private rows:MomentumInput[]=[];
 reset(){this.rows=[];}
 feed(x:MomentumInput):MomentumRead {
  const last=this.rows.at(-1);
  if(last&&(last.symbol!==x.symbol||last.session!==x.session||x.at-last.at>90000))this.reset();
  if(!x.ready||![x.at,x.price,x.direction].every(Number.isFinite)||x.price<=0){this.reset();return pending(x.reason);}
  if(this.rows.at(-1)&&x.at<=this.rows.at(-1)!.at)return pending('Esperando una observación nueva');
  this.rows.push({...x});this.rows=this.rows.slice(-12);
  // Compare two observed rolling 5-minute flow windows at least 5 minutes apart.
  const prior=this.rows.slice().reverse().find(r=>x.at-r.at>=300000&&x.at-r.at<=360000);
  if(!prior||x.flow==null||prior.flow==null||!Number.isFinite(x.flow)||!Number.isFinite(prior.flow))return pending('Acumulando dos ventanas de prima no superpuestas');
  if(![1,-1].includes(x.direction))return pending('Dirección actual sin predominio');
  const acceleration=(x.flow-prior.flow)*x.direction;
  const ahead=x.target==null?null:(x.target-x.price)*x.direction;
  const risk=x.stop==null?null:(x.price-x.stop)*x.direction;
  const space=ahead!=null&&risk!=null&&ahead>0&&risk>0?ahead/risk:null;
  const aligned=x.priceAligned&&x.flow*x.direction>0;
  const persistence=this.rows.slice(-3).length===3&&this.rows.slice(-3).every(r=>r.direction===x.direction&&r.priceAligned&&r.flow!=null&&r.flow*r.direction>0);
  const volume=x.volumeRatio!=null&&Number.isFinite(x.volumeRatio)&&x.volumeRatio>=1;
  const score=Math.round((aligned?30:0)+Math.max(0,Math.min(1,acceleration/.25))*25+(volume?15:0)+(space!=null&&space>=1.5?20:0)+(persistence?10:0));
  const extended=ahead!=null&&ahead>0&&x.atr!=null&&x.atr>0&&ahead<x.atr*.5;
  const state=!aligned?'NO CONFIRMA':extended?'EXTENDIDO':acceleration<-.1?'PIERDE FUERZA':persistence&&x.routeConfirmed&&space!=null&&space>=1.5&&volume&&acceleration>=.05?'ACTIVO':'PREPARÁNDOSE';
  return {side:x.direction===1?'CALL':'PUT',state,score,acceleration,space,reason:state==='ACTIVO'?'Presión acelerando, precio persistente y recorrido ≥ 1.5 veces la invalidación':state==='EXTENDIDO'?'Menos de medio ATR hasta la referencia':state==='PIERDE FUERZA'?'Presión disminuyendo; no confirma giro contrario':state==='NO CONFIRMA'?'Precio y prima no coinciden':'Falta aceleración, continuación, volumen o recorrido verificable'};
 }
}
