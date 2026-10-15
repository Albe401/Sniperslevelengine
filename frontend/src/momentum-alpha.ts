/** Learning layer. Closed candles only; scores are evidence, never calibrated alpha. */
export interface AlphaBar {timestampMs:number;open:number;high:number;low:number;close:number;volume:number}
export interface AlphaPath {ready:boolean;clear:boolean;obstacle:number|null;space:number|null;reason:string}
export function alphaPath(bars:AlphaBar[],at:number,price:number,direction:number,target:number|null,stop:number|null,atr:number|null):AlphaPath {
 const pending=(reason:string):AlphaPath=>({ready:false,clear:false,obstacle:null,space:null,reason});
 if(![at,price,direction].every(Number.isFinite)||price<=0||![1,-1].includes(direction)||target==null||stop==null||atr==null||![target,stop,atr].every(Number.isFinite)||atr<=0)return pending('Faltan precio, objetivo, invalidación o ATR');
 const rows=bars.filter(b=>b.timestampMs+60000<=at).slice(-30);
 if(rows.length<6||at-rows.at(-1)!.timestampMs-60000>90000||rows.some((b,i)=>![b.timestampMs,b.open,b.high,b.low,b.close,b.volume].every(Number.isFinite)||b.low<=0||b.volume<0||b.high<Math.max(b.open,b.close)||b.low>Math.min(b.open,b.close)||(i>0&&b.timestampMs-rows[i-1].timestampMs!==60000)))return pending('Velas cerradas recientes sin continuidad suficiente');
 const risk=(price-stop)*direction,reward=(target-price)*direction;
 if(risk<=0||reward<=0)return pending('Objetivo o invalidación en el lado incorrecto');
 const tolerance=atr*.05,zones:number[]=[];
 // A pivot needs a subsequent closed candle moving away: no future confirmation.
 for(let i=1;i<rows.length-1;i++){
  const a=rows[i-1],b=rows[i],c=rows[i+1];
  const level=direction===-1?b.low:b.high;
  const pivot=direction===-1?b.low<=a.low&&b.low<=c.low&&(b.low<a.low||b.low<c.low)&&c.close-b.low>=atr*.15:b.high>=a.high&&b.high>=c.high&&(b.high>a.high||b.high>c.high)&&b.high-c.close>=atr*.15;
  if(!pivot)continue;
  // A later decisive close through the zone invalidates its former obstruction.
  const broken=rows.slice(i+1).some(r=>direction===-1?r.close<level-tolerance:r.close>level+tolerance);
  const distance=(level-price)*direction;
  if(!broken&&distance>=-tolerance&&distance<=reward)zones.push(level);
 }
 const obstacle=zones.sort((a,b)=>Math.max(0,(a-price)*direction)-Math.max(0,(b-price)*direction))[0]??null;
 const available=obstacle==null?reward:Math.max(0,(obstacle-price)*direction-tolerance);
 const space=available/risk,clear=space>=1.5&&available>=atr*.5;
 return {ready:true,clear,obstacle,space,reason:clear?'Recorrido suficiente en las velas observadas':obstacle==null?'Recorrido insuficiente hasta el objetivo':`Rebote/rechazo reciente en ${obstacle.toFixed(2)} corta el recorrido`};
}
export interface AlphaRead {version:2;score:number|null;state:string;path:AlphaPath;rvolChange:number|null;volumeChange:number|null;reason:string}
export function momentumAlpha(momentum:{score:number|null;state:string;acceleration:number|null},path:AlphaPath,rvol:number|null,previousRvol:number|null,volume:number|null,previousVolume:number|null):AlphaRead {
 const change=(a:number|null,b:number|null)=>a!=null&&b!=null&&Number.isFinite(a)&&Number.isFinite(b)&&a>=0&&b>=0?a-b:null;
 const rvolChange=change(rvol,previousRvol),volumeChange=change(volume,previousVolume);
 const growth=rvolChange??volumeChange;
 const ready=momentum.score!=null&&path.ready&&growth!=null;
 // The original score already contains flow/direction: do not count it again.
 const score=ready?Math.round(momentum.score!*.7+(path.clear?20:0)+(growth!>0?10:0)):null;
 const state=!path.ready?'NO CONFIRMA':!path.clear?'CAMINO BLOQUEADO':!ready?'PREPARÁNDOSE':momentum.state==='ACTIVO'&&growth!>0?'ACTIVO':momentum.state==='ACTIVO'?'PREPARÁNDOSE':momentum.state;
 return {version:2,score,state,path,rvolChange,volumeChange,reason:!path.clear?path.reason:rvolChange==null?'RVOL comparable pendiente; crecimiento usa volumen local, no RVOL': 'Comparación experimental de aceleración, RVOL y recorrido observado'};
}
