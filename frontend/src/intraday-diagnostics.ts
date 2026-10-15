import type {Candle} from './model';
export interface LevelSample {key:string;name:string;level:number;score:number;at:number}
export interface DiagnosticInput {session:string;ready:boolean;at:number;spot:number|null;target:number|null;net:number;flow:number|null;bars:Candle[];levels:LevelSample[]}
export interface Diagnostics {levels:string[];approach:string;distance:number|null;exhaustion:string;reasons:string[]}
const pending=():Diagnostics=>({levels:[],approach:'ESPERANDO PRECIO Y DIRECCIÓN VIGENTES',distance:null,exhaustion:'EVIDENCIA INSUFICIENTE',reasons:[]});
/** Stateful comparisons only within the same session/level and timestamp order. */
export class IntradayDiagnostics {
 private session='';private at=0;
 private levels=new Map<string,LevelSample>();private trends=new Map<string,string>();
 private price?:{at:number;spot:number;target:number;distance:number};
 private flow?:{at:number;value:number};
 update(x:DiagnosticInput):Diagnostics {
  if(x.session!==this.session||x.at<this.at||x.at-this.at>15000||!x.ready){this.levels.clear();this.trends.clear();this.price=undefined;this.flow=undefined;}
  this.session=x.session;this.at=x.at;
  const out=pending();if(!x.ready)return out;
  const keys=new Set(x.levels.map(l=>l.key));
  for(const key of this.levels.keys())if(!keys.has(key)){this.levels.delete(key);this.trends.delete(key);}
  for(const l of x.levels){
   const prev=this.levels.get(l.key);
   if(!prev||l.at<prev.at||l.at-prev.at>900000){this.trends.set(l.key,'ESPERANDO OTRO SNAPSHOT');this.levels.set(l.key,l);}
   else if(l.at>prev.at){
    const d=l.score-prev.score;
    this.trends.set(l.key,`${d>=5?'GANANDO PESO':d<=-5?'PERDIENDO PESO':'SIN CAMBIO RELEVANTE'} · ${d>=0?'+':''}${d.toFixed(1)} pts`);
    this.levels.set(l.key,l);
   }
   out.levels.push(`${l.name} $${l.level.toFixed(2)} · ${l.score.toFixed(1)}/100 · ${this.trends.get(l.key)}`);
  }
  if(x.spot!=null&&x.target!=null&&Math.abs(x.net)>=.15){
   const distance=Math.abs(x.target-x.spot);out.distance=distance;
   const p=this.price;
   const tolerance=x.spot*.0003;
   out.approach=distance<=tolerance?'EN LA ZONA DEL NIVEL':!p||p.target!==x.target?'ESPERANDO DESPLAZAMIENTO':p.distance-distance>tolerance?'ACERCÁNDOSE AL NIVEL':distance-p.distance>tolerance?'ALEJÁNDOSE DEL NIVEL':'SIN AVANCE RELEVANTE';
   if(!p||p.target!==x.target||x.at-p.at>=10000)this.price={at:x.at,spot:x.spot,target:x.target,distance};
  }else this.price=undefined;
  const b=x.bars.slice(-4),direction=Math.sign(x.net);
  const contiguous=b.length===4&&b.every((v,i)=>i===0||v.timestampMs-b[i-1].timestampMs===120000);
  if(contiguous&&Math.abs(x.net)>=.15&&x.flow!=null){
   const last=b[3],range=last.high-last.low;
   const progress=(b[3].close-b[2].close)*direction;
   const prior=(b[2].close-b[1].close)*direction;
   const shrinking=prior>0&&progress>=0&&progress<prior*.4;
   const wick=range>0&&(direction>0?(last.high-Math.max(last.open,last.close))/range:(Math.min(last.open,last.close)-last.low)/range)>=.5;
   const rejection=direction>0?last.close<last.open:last.close>last.open;
   const f=this.flow;
   const weakening=!!f&&x.at-f.at>=10000&&x.flow*direction<f.value*direction-.15;
   const opposing=x.flow*direction<-.15;
   if(shrinking)out.reasons.push('Avance menor en velas cerradas de 2m');
   if(wick&&rejection)out.reasons.push('Mecha de rechazo y cierre contra el movimiento');
   if(weakening)out.reasons.push('Flujo 1m pierde apoyo frente a la observación previa');
   if(opposing)out.reasons.push('Flujo 1m contrario al control');
   out.exhaustion=(shrinking||wick&&rejection)&&(weakening||opposing)?'POSIBLE AGOTAMIENTO · SIN GIRO CONFIRMADO':out.reasons.length?'SEÑALES PARCIALES · VIGILAR':'SIN CONFLUENCIA DE AGOTAMIENTO';
  }
  if(x.flow!=null&&(!this.flow||x.at-this.flow.at>=10000))this.flow={at:x.at,value:x.flow};
  if(x.flow==null)this.flow=undefined;
  return out;
 }
}
