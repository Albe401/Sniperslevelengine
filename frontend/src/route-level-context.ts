import {dailyEvidenceContracts,evidenceByStrike} from './level-evidence';
import type {LiveState} from './live';
import {interpretationFresh} from './confirmations';
import {rangeFresh} from './daily-range';
import {displayStructure,mapHorizon} from './reading-display';
import type {RouteLevel} from './route-engine';
export type LevelRole='call-wall'|'put-wall'|'magnet'|'barrier'|'pull'|'vacuum-edge';
export interface LevelAssessment {name:string;price:number;role:LevelRole|undefined;strength:number|null;grade:string;change:number|null;provisional:boolean;evidence?:import('./level-evidence').LevelEvidence}
export interface RouteStructure {quality:'limited'|'unverified'|'supported';requiresAdditionalTest:boolean;confirmationBlocked?:boolean;backing:LevelAssessment|null;anchor:LevelAssessment|null;ahead:LevelAssessment[];levels:LevelAssessment[];reason:string}
const score=(n:unknown):number|null=>typeof n==='number'&&Number.isFinite(n)&&n>=0&&n<=100?n:null;
const change=(n:unknown):number|null=>typeof n==='number'&&Number.isFinite(n)?n:null;
/** Same snapshot scores as the important-level map; dynamics stays a separate source. */
export function mapRouteLevels(s:LiveState|undefined,symbol:string,active:boolean,now:number):RouteLevel[]{
 if(!active||s?.symbol!==symbol||!interpretationFresh(s.publishedMs,now)||s.optionSnapshotStatus?.phase!=='ready'||!rangeFresh(s.optionSnapshotStatus,now))return [];
 const candidates=s.optionSnapshotStatus.horizons.filter(h=>h.mode==='DAY');
 const h=candidates.length===1?candidates[0]:undefined;
 if(!h||!h.structure||h.expirations?.length!==1||h.observedMs!==s.optionSnapshotStatus.observedMs||!rangeFresh({phase:'ready',observedMs:h.observedMs},now))return [];
 const structure=displayStructure(h,true),expiration=h.expirations.slice().sort()[0];
 if(!structure||!expiration||expiration!==new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'}))return [];
 const provisional=!!h.structure.provisional;
 const today=new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
 const dynamics=expiration===today&&s.levelDynamics?.symbol===symbol&&s.levelDynamics.available&&s.levelDynamics.at<=now&&now-s.levelDynamics.at<=3000?s.levelDynamics:undefined;
 const levels:RouteLevel[]=[];
 const contracts=dailyEvidenceContracts(s,symbol,active,now);
 const concentrations=contracts?evidenceByStrike(contracts,h.observedMs):undefined;
 const add=(name:string,price:number|null|undefined,role:LevelRole,value?:number|null)=>{
  if(typeof price!=='number'||!Number.isFinite(price)||price<=0)return;
  const d=dynamics?.levels.find(l=>l.name===name&&l.strike===price);
  levels.push({name,price,role,observed:now,asof:h.observedMs,expiration,provisional,strength:score(value),
   change:d&&!d.provisional?change(d.change):null,dynamicStrength:score(d?.strength),source:'important-level-map',evidence:concentrations?.get(price)});
 };
 add('CALL WALL',structure.callWall?.strike,'call-wall',structure.callWall?.score);
 add('PUT WALL',structure.putWall?.strike,'put-wall',structure.putWall?.score);
 add('IMÁN PRINCIPAL',structure.selected.M1,'magnet');add('IMÁN DE LIQUIDEZ',structure.magnet,'magnet');
 for(const route of structure.routes){
  add(`BARRERA ${route.direction}`,route.barrier,'barrier',route.barrierScore);
  add(`${route.direction} PULL`,route.pullTarget,'pull',route.score);
  add(`VACÍO ${route.direction} · INFERIOR`,route.vacuumLow,'vacuum-edge',route.vacuumScore);
  add(`VACÍO ${route.direction} · SUPERIOR`,route.vacuumHigh,'vacuum-edge',route.vacuumScore);
 }
 return levels;
}
/** Strength is a model score, not probability or proof that price must reverse. */
export function assessRouteStructure(levels:RouteLevel[],spot:number,direction:number,anchorPrice:number|null,now:number):RouteStructure {
 const usable=levels.filter(l=>l.price>0&&Number.isFinite(l.price)&&l.observed<=now&&(l.asof??l.observed)<=now&&now-(l.asof??l.observed)<=900000);
 const assess=(l:RouteLevel):LevelAssessment=>{const strength=score(l.strength);return {name:l.name,price:l.price,role:l.role,strength,grade:strength==null?'pendiente':strength>=80?'muy alta':strength>=60?'alta':strength>=40?'media':'baja',change:change(l.change),provisional:l.provisional,evidence:l.evidence};};
 const all=usable.map(assess);
 const anchor=all.find(l=>l.price===anchorPrice&&l.role!=='vacuum-edge')??null;
 const role=direction===1?'put-wall':direction===-1?'call-wall':null;
 const backing=role?all.filter(l=>l.role===role&&(spot-l.price)*direction>=0).sort((a,b)=>Math.abs(a.price-spot)-Math.abs(b.price-spot))[0]??null:null;
 const relevant=[anchor,backing].filter((l):l is LevelAssessment=>l!=null&&['call-wall','put-wall','barrier'].includes(l.role??''));
 const limited=relevant.some(l=>l.evidence?.quality==='partial'||l.strength!=null&&l.strength<60||l.change!=null&&l.change<=-5);
 const supported=relevant.length>0&&relevant.every(l=>l.strength!=null&&l.strength>=60&&!l.provisional&&(l.change==null||l.change>-5));
 return {quality:limited?'limited':supported?'supported':'unverified',requiresAdditionalTest:limited,confirmationBlocked:limited,
  backing,anchor,ahead:direction===0?[]:all.filter(l=>(l.price-spot)*direction>spot*.0003).sort((a,b)=>Math.abs(a.price-spot)-Math.abs(b.price-spot)),levels:all,
  reason:limited?'Respaldo estructural limitado o perdiendo fuerza; otra vela no verifica el respaldo del nivel':supported?'Niveles de respaldo fuertes en el modelo; la reacción del precio sigue siendo necesaria':'Fuerza del respaldo pendiente o provisional; no asumir soporte confirmado'};
}
