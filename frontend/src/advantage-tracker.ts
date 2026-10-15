import {intradaySummary} from './intraday-summary';
export type Advantage=ReturnType<typeof intradaySummary>;
/** Display-only persistence. It neither arms the bot nor changes execution gates. */
export class AdvantageTracker {
 private memory?:{session:string;direction:number;key:string;since:number;at:number;confirmed:boolean};
 update(raw:Advantage,session:string,key:string,now:number):Advantage {
  if(!raw.direction){this.memory=undefined;return raw;}
  let m=this.memory;
  if(!m||m.session!==session||m.direction!==raw.direction||now<m.at||now-m.at>15000){m={session,direction:raw.direction,key,since:now,at:now,confirmed:false};}
  else if(key!==m.key&&now-m.since>=2000){m.confirmed=true;m.key=key;}
  m.at=now;this.memory=m;
  if(m.confirmed)return raw;
  const who=raw.direction>0?'compradora':'vendedora';
  return {...raw,headline:`VENTAJA ${who.toUpperCase()} EN FORMACIÓN`,direction:0,tone:'waiting',evidence:`Presión ${who} con apoyo; falta persistencia en nuevas observaciones.`};
 }
}
