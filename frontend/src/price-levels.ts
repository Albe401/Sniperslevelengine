import type {LiveState} from './live';
import type {PriceNow} from './price-now';
import {interpretationFresh} from './confirmations';
import {rangeFresh} from './daily-range';
import {currentMapHorizon,displayStructure} from './reading-display';
import {price,escapeHtml as esc} from './model';
import {currentSessionCandles} from './candle-series';
export interface PriceLevels {dominant:string;nearest:string;route:string;condition:string;quality:string;important:string;reaction:string;narrative:string}
const positive=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n)&&n>0;
const score=(n:unknown):n is number=>typeof n==='number'&&Number.isFinite(n)&&n>=0&&n<=100;
export function readPriceLevels(s:LiveState|undefined,symbol:string,active:boolean,p:PriceNow,now=Date.now()):PriceLevels {
 const out:PriceLevels={dominant:'Sin puntuaciones comparables',nearest:'Esperando precio y niveles',route:'Sin objetivo estructural disponible',condition:'Esperando referencias para definir condición',quality:'Estructura pendiente',important:'Sin niveles vigentes',reaction:'Sin defensa o rechazo reciente confirmado por las velas',narrative:'Esperando precio vigente y referencias'};
 if(!active||s?.symbol!==symbol||!interpretationFresh(s.publishedMs,now))return out;
 const today=new Date(now).toLocaleDateString('en-CA',{timeZone:'America/New_York'});
 let bars=currentSessionCandles(s.chartHistory,1,now).filter(b=>b.complete&&b.sessionDate===today).slice(-30);
 for(let i=bars.length-1;i>0;i--)if(bars[i].timestampMs-bars[i-1].timestampMs!==60000){bars=bars.slice(i);break;}
 if(!bars.length||now-bars.at(-1)!.timestampMs>150000)bars=[];
 const levels:{name:string;value:number;score?:number}[]=[];
 const add=(name:string,value:unknown,weight?:unknown)=>{if(positive(value))levels.push({name,value,score:score(weight)?weight:undefined});};
 let defended:{value:number;direction:number;at:number}|undefined;
 if(p.ready&&positive(p.spot))for(let i=1;i<bars.length-1;i++){
  const b=bars[i],before=bars[i-1],after=bars[i+1],tol=p.spot*.0003;
  if(b.low<before.low&&b.low<after.low){
   add('PISO OBSERVADO',b.low);
   if(now-b.timestampMs<=600000&&after.close>b.low+tol&&p.spot>b.low+tol&&!bars.slice(i+1).some(x=>x.close<b.low))defended={value:b.low,direction:1,at:b.timestampMs};
  }
  if(b.high>before.high&&b.high>after.high){
   add('TECHO OBSERVADO',b.high);
   if(now-b.timestampMs<=600000&&after.close<b.high-tol&&p.spot<b.high-tol&&!bars.slice(i+1).some(x=>x.close>b.high))defended={value:b.high,direction:-1,at:b.timestampMs};
  }
 }
 if(defended){out.reaction=`${defended.direction>0?'REBOTE DESDE':'RECHAZO EN'} $${price(defended.value)} · ${new Date(defended.at).toLocaleTimeString('es',{timeZone:'America/New_York'})} ET`;out.narrative=`${defended.direction>0?'La caída se frenó y el precio rebotó':'La subida se frenó y el precio retrocedió'} en $${price(defended.value)}. ${p.direction===defended.direction?(defended.direction>0?'Los compradores recuperan terreno por reacción del precio; falta contrastarlo con opciones.':'Los vendedores recuperan terreno por reacción del precio; falta contrastarlo con opciones.'):'El movimiento reciente ya no acompaña esa reacción.'} Absorción no confirmada.`;}
 const snap=s.optionSnapshotStatus;
 const h=currentMapHorizon(snap,now);
 const st=displayStructure(h,snap?.phase==='ready'&&rangeFresh(snap,now));
 const provisional=!!h?.structure.provisional;
 if(h&&st)out.quality=`${provisional?'PROVISIONAL · cadena incompleta':'ESTRUCTURA COMPLETA'} · ${h.mode==='DAY'?'Hoy':'Próximo vencimiento'} · ${(h.expirations??[]).join(', ')}`;
 else if(levels.length)out.quality='Solo giros observados en velas de hoy · estructura de opciones pendiente';
 add('PUT WALL',st?.putWall?.strike,st?.putWall?.score);add('IMÁN PRINCIPAL',st?.selected.M1);add('CALL WALL',st?.callWall?.strike,st?.callWall?.score);
 // Rank only like-for-like wall scores. Magnet attraction and vacuum scores have different meanings.
 const walls=levels.filter(l=>l.score!=null).sort((a,b)=>b.score!-a.score!);
 const fmt=(l:typeof levels[number])=>`${l.name} $${price(l.value)}`;
 if(walls.length){const best=walls[0],ties=walls.filter(l=>l.score===best.score);out.dominant=ties.map(fmt).join(' / ')+` · ${best.score!.toFixed(1)}/100${ties.length>1?' · empate':''}`;}
 out.important=levels.filter(l=>!l.name.includes('OBSERVADO')).sort((a,b)=>a.value-b.value).map(l=>fmt(l)+(l.score!=null?` (${l.score.toFixed(1)}/100)`:' (sin score comparable)')).join(' · ')||out.important;
 if(!positive(p.spot))return out;
 const spot=p.spot;
 if(walls.length)out.dominant+=` · ${walls[0].value>spot?'sobre el precio':walls[0].value<spot?'bajo el precio':'en el precio'}`;
 for(const r of st?.routes??[])if(provisional||r.structuralReady){
  add(`BARRERA ${r.direction}`,r.barrier);
  add(`OBJETIVO ${r.direction}`,r.target);
 }
 const unique=levels.filter((l,i,a)=>a.findIndex(x=>x.value===l.value)===i);
 const nearest=[...unique].sort((a,b)=>Math.abs(a.value-spot)-Math.abs(b.value-spot))[0];
 if(nearest)out.nearest=`${fmt(nearest)} · ${price(Math.abs(nearest.value-spot))} puntos · ${nearest.value>spot?'resistencia potencial':nearest.value<spot?'soporte potencial':'precio en el nivel'}`;
 if(!p.ready||!p.direction){out.route='Sin ruta direccional: precio lateral o lectura pendiente';return out;}
 const d=p.direction;
 const ahead=unique.filter(l=>(l.value-spot)*d>0).sort((a,b)=>Math.abs(a.value-spot)-Math.abs(b.value-spot));
 const behind=unique.filter(l=>(l.value-spot)*d<0).sort((a,b)=>Math.abs(a.value-spot)-Math.abs(b.value-spot));
 if(!ahead.length){out.route=`Sin ${d>0?'resistencia':'soporte'} mapeado en la dirección actual`;out.condition='No se proyecta un precio fuera de los niveles disponibles';return out;}
 out.route=`Primera referencia: ${fmt(ahead[0])} · ${price(Math.abs(ahead[0].value-spot))} puntos`;
 out.condition=`Solo si ${d>0?'supera y sostiene':'pierde y permanece bajo'} $${price(ahead[0].value)}${ahead[1]?`, siguiente referencia ${fmt(ahead[1])}`:': siguiente nivel no disponible'}.`;
 if(behind[0])out.condition+=` Escenario cuestionado si ${d>0?'pierde':'recupera'} $${price(behind[0].value)}.`;
 if(defended&&defended.direction===d)out.condition+=` La defensa de $${price(defended.value)} falla si ${d>0?'cierra una vela de 1m por debajo':'cierra una vela de 1m por encima'}.`;
 return out;
}
export function priceLevelsPanel(r:PriceLevels):string {
 return `<section class="panel"><div class="panel-head"><h2>Niveles importantes y recorrido posible</h2></div><p>${esc(r.quality)}</p><p><b>${esc(r.reaction)}</b></p><p>${esc(r.narrative)}</p><p><b>Mayor peso entre muros:</b> ${esc(r.dominant)}</p><p><b>Nivel cercano:</b> ${esc(r.nearest)}</p><p><b>Recorrido posible:</b> ${esc(r.route)}</p><p>${esc(r.condition)}</p><p>${esc(r.important)}</p><small>El peso estructural no demuestra quién controla el precio. Las referencias son condicionales; no garantizan llegada, rebote ni tiempo.</small></section>`;
}
