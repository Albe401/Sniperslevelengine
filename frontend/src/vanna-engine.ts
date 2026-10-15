import {absorptionReading} from './absorption-reading';
import type {LiveState} from './live';
import type {ControlReading} from './intraday-control';
import {vannaConfluence,type VannaRead} from './vanna-confluence';
import {structuralCards} from './structural-map-preview';
export interface VannaPair {call:VannaRead;put:VannaRead}
export interface VannaEngine {at:number;symbol:string;levels:{name:string;strike:number;expiration:string;support:VannaPair}[];structure:{name:string;expirations:string[];support:VannaPair}[];absorption:VannaPair;route:VannaPair;dealers:VannaPair;limitation:string}
/** Scoped sensitivity evidence; never grants price/flow confirmation. No UI markup. */
export function vannaEngine(s:LiveState|undefined,r:ControlReading,active:boolean,now:number):VannaEngine {
 const pair=(expirations:string[],strike?:number):VannaPair=>({call:vannaConfluence(s,{call:100,put:0,side:'CALL',state:'',reason:'',components:[]},r.symbol,active,now,{expirations,strike}),put:vannaConfluence(s,{call:0,put:100,side:'PUT',state:'',reason:'',components:[]},r.symbol,active,now,{expirations,strike})});
 const cards=structuralCards(s,r.symbol,active,now);
 const structure=cards.map((card,i)=>{const modes=i===0?['DAY','NEXT_SESSION']:i===1?['WEEK']:['NEXT4'];const horizons=s?.optionSnapshotStatus?.horizons.filter(h=>modes.includes(h.mode))??[];const expirations=card.side!=='PENDIENTE'&&horizons.length===1? horizons[0].expirations:[];return {name:card.name,expirations,support:pair(expirations)};});
 const levels=(r.strikeReading?.rows??[]).map(l=>({name:l.name,strike:l.strike,expiration:l.expiration,support:pair([l.expiration],l.strike)}));
 const nowExpiry=new Intl.DateTimeFormat('en-CA',{timeZone:'America/New_York'}).format(now);
 const assessment=absorptionReading(r,now);
 const absorption=assessment.kind!=='waiting'?levels.find(l=>l.name===assessment.name&&l.strike===assessment.level)?.support:undefined;
 const route=r.dominant?levels.find(l=>l.strike===r.dominant!.level)?.support:undefined;
 return {at:now,symbol:r.symbol,levels,structure,absorption:absorption??pair([]),route:route??pair([]),dealers:pair([nowExpiry]),limitation:'Muestra parcial, hasta cuatro expiraciones y 64 contratos por expiración. Vanna explica cambios locales de delta por IV; no confirma defensa, ruptura, absorción, flujo agresor, inventario dealer ni dirección. No recalifica fuerza, objetivos, ATR ni porcentajes de exposición.'};
}
