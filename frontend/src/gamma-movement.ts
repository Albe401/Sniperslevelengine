import type {ControlReading} from './intraday-control';
import {intradaySummary} from './intraday-summary';
export interface GammaMovement {kind:string;headline:string;detail:string;direction:number;bias:number}
/** Descriptive price/flow transitions. Gamma strength never votes for direction. */
export class GammaMovementTracker {
 private memory?:{session:string;at:number;confirmed:number;candidate?:{direction:number;since:number;key:string};transition?:{direction:number;at:number}};
 update(r:ControlReading,now=Date.now()):GammaMovement {
  const p=r.priceNow,f=r.flowControl,session=`${r.symbol}:${r.session}`;
  const common=r.marketEvidence;
  const raw=common?{direction:common.direction,headline:common.score.state==='Lectura mixta'?'LECTURA COMÚN · MIXTA, SIN PREDOMINIO':`LECTURA COMÚN · ${common.score.side}`,evidence:common.detail}:intradaySummary(p,r.priceLevels,f);
  const result=(kind:string,headline:string,detail:string,direction=0):GammaMovement=>({kind,headline,detail,direction,bias:direction*32});
  if(!Number.isFinite(now)||!p?.ready||(common?common.score.call==null:f?.quality!=='ready'||!f.sourceKey)){this.memory=undefined;return result('waiting','LECTURA PENDIENTE',raw.evidence);}
  let m=this.memory;
  if(!m||m.session!==session||now<m.at||now-m.at>15000)m={session,at:now,confirmed:0};
  m.at=now;this.memory=m;
  const d=raw.direction,side=d>0?'CALL':'PUT';
  if(!d){m.candidate=undefined;return result('mixed',raw.headline,raw.evidence);}
  // A slower advance is exhaustion evidence, not confirmation of the opposite side.
  if(p.pace?.includes('PERDIENDO VELOCIDAD')){m.candidate=undefined;return result('slowing',d>0?'LA SUBIDA PIERDE FUERZA':'LA CAÍDA PIERDE FUERZA','El avance se ralentiza en velas cerradas; todavía no confirma un giro.');}
  const key=common?.key??JSON.stringify([p.at,f?.sourceKey]);
  if(m.confirmed!==d){
   let c=m.candidate;
   if(!c||c.direction!==d)c=m.candidate={direction:d,since:now,key};
   if(key===c.key||now-c.since<2000)return result('forming',`CAMBIANDO HACIA ${side} · EN OBSERVACIÓN`,'Precio y flujo coinciden; falta persistencia en nuevas observaciones.');
   const reversed=m.confirmed===-d;m.confirmed=d;m.candidate=undefined;m.transition=reversed?{direction:d,at:now}:undefined;
  }else m.candidate=undefined;
  const changed=m.transition?.direction===d&&now-m.transition.at<=15000;
  return result(changed?'reversal':'aligned',changed?`CAMBIO DE SESGO ${d>0?'PUT A CALL':'CALL A PUT'} · OBSERVADO`:`SESGO ${side} · PRECIO Y FLUJO ALINEADOS`,raw.evidence+' La persistencia de 2 s describe el sesgo; la activación requiere una prueba y continuación del precio.',d);
 }
}
