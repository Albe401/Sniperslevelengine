import {levelOpenInterest} from './level-open-interest';
import {sameStrike,strikeKey} from './strike-key';
import {mapDataQuality} from './map-data-quality';
import {levelQuality} from './level-quality';
import {quantitativeLevel,levelDefense} from './level-quantitative';
import type {EtfReport} from './etf-dealers';
import {sharedLevelDetail} from './shared-level-detail';
import {barrierFlow} from './barrier-flow';
import {readLevelActivities,type LevelActivityInput} from './level-activity';
import {activityStyle,activityChips} from './level-activity-view';
import './level-activity.css';
import type {BidAskWindow} from './strike-bid-ask';
import {dailyDistance} from './atr-context';
import {atrDistance,type AtrRead} from './intraday-atr';
import {directionLevelsPanel,directionLevelTag,type DirectionLevels} from './direction-levels';
import type {controlOutlook} from './control-outlook';
import {displayStructure} from './reading-display';
import {escapeHtml as esc,price} from './model';
import type {Horizon} from './live';
import type {ContractWindow} from './flow-coverage';

export function levelVolume(rows:ContractWindow[],strike:number,expirations:string[],minutes:number,fresh:boolean):number|null {
  return indexedLevelVolume(rows)(strike,expirations,minutes,fresh);
}
export function indexedLevelVolume(rows:ContractWindow[]){
 const index=new Map<string,ContractWindow[]>();
 const key=(expiration:string,strike:number,kind:string,minutes:number)=>JSON.stringify([expiration,strikeKey(strike),kind.toUpperCase(),minutes]);
 for(const row of rows){if(strikeKey(row.strike)==null)continue;const k=key(row.expiration,row.strike,row.kind,row.minutes);const old=index.get(k);if(old)old.push(row);else index.set(k,[row]);}
 return (strike:number,expirations:string[],minutes:number,fresh:boolean):number|null=>{
  if(strikeKey(strike)==null)return null;
  if(!fresh||!expirations.length)return null;
  let total=0;
  for(const expiration of new Set(expirations))for(const kind of ['CALL','PUT']) {
    const matches=index.get(key(expiration,strike,kind,minutes))??[];
    // Both sides must have a complete observed window; missing is never zero.
    if(matches.length!==1||!matches[0].covered||matches[0].contracts==null||!Number.isFinite(matches[0].contracts)||matches[0].contracts<0)return null;
    total+=matches[0].contracts;
  }
  return Number.isFinite(total)?total:null;
 };
}
const formatVolume=(n:number|null)=>n==null?'—':Intl.NumberFormat('es',{maximumFractionDigits:2}).format(n);
export function levelStrength(value:number|null|undefined,vacuum=false,provisional=false,label='',context:{oi?:ReturnType<typeof levelOpenInterest>;caption?:string;detail?:string}={}):string {
  const valid=value!=null&&Number.isFinite(value)&&value>=0&&value<=100;
  const title=vacuum?'INTENSIDAD DEL VACÍO':label.includes('PULL')?'FUERZA · PULL SCORE':label.startsWith('BARRERA')?'FUERZA · BARRIER SCORE':label.startsWith('IMÁN')?'FUERZA · MAGNET SCORE':label.includes('WALL')?'FUERZA · WALL SCORE':'FUERZA';
  const grade=valid?(value>=80?'MUY ALTA':value>=60?'ALTA':value>=40?'MEDIA':'BAJA'):'';
  const oi=context.oi;
  const scoreTrack=valid?`<div class="level-strength-track" role="meter" aria-label="${esc(label?label+' · ':'')}${title}" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${value}"><i style="width:${value}%"></i></div>`:'';
  const oiTrack=oi&&!oi.zeroOi&&oi.callPct!=null&&oi.putPct!=null?`<div class="level-strength-track level-oi-track" role="img" aria-label="OI CALL ${oi.callPct}% / PUT ${oi.putPct}%" title="${esc(context.detail??'')} "><i class="oi-call" style="width:${oi.callPct}%"></i><i class="oi-put" style="width:${oi.putPct}%"></i></div>`:'';
  return `<div class="level-strength" title="${esc(label?label+' · ':'')}${title}: concentración relativa del modelo dentro de la cadena consultada, no probabilidad de rebote; no comparar scores entre tipos de nivel.${provisional?' Estructura provisional; cadena incompleta.':''}"><span>${title} <b>${valid?value.toFixed(1)+'/100 · '+grade+(provisional?' · PARCIAL':''):'Sin datos'}</b>${context.caption?` · <small class="wall-oi-count">${esc(context.caption)}</small>`:''}</span>${scoreTrack}${oiTrack}</div>`;
}
export type MapOutlook=ReturnType<typeof controlOutlook>;
export function mapLevelReading(labels:string[],strike:number|undefined,outlook?:MapOutlook,barrierCaption?:string,barrierAction?:string,barrierEvidence?:string):string {
 if(!outlook)return barrierEvidence?`<details class="map-level-reading" data-horizon-details="map-reading-${esc(labels.join('-'))}"><summary><span>${esc(barrierCaption??'Lectura cuantitativa')}</span><b>${esc(barrierAction??'Lectura +')}</b></summary><div class="map-reading-detail"><p>${esc(barrierEvidence)}</p></div></details>`:'';
 const row=strike==null?undefined:outlook.rows.find(r=>sameStrike(r.level,strike)&&labels.includes(r.name));
 return `<details class="map-level-reading" data-horizon-details="map-reading-${esc(labels.join('-'))}"><summary><span>${esc(barrierCaption??row?.state??'Esperando interacción')}</span><b>${esc(barrierAction??'Lectura +')}</b></summary><div class="map-reading-detail">${barrierEvidence?`<p>${esc(barrierEvidence)}</p>`:""}${row?`<p>${esc(row.quality)} · ${esc(row.state)}</p>`:''}<p class="map-reading-scope">Contexto intradía compartido</p>${sharedLevelDetail(outlook.marketEvidence,strike)}${[['Nivel decisivo',outlook.level,outlook.levelDetail],['Qué está cambiando',outlook.change,outlook.changeDetail],['Recorrido',outlook.route,outlook.routeDetail],['Qué cambia la lectura',outlook.invalidate,outlook.invalidateDetail]].map(([label,value,detail])=>`<p><b>${esc(label)}</b><span>${esc(value)}</span>${esc(detail)}</p>`).join('')}</div></details>`;
}
export function dayMap(horizon:Horizon|undefined,structureFresh:boolean,rows:ContractWindow[],flowFresh:boolean,currentPrice:number|null|undefined,spotHtml='',outlook?:MapOutlook,routeDesign=false,directionHtml='',linkedLevels?:DirectionLevels,atr?:AtrRead,headerContext="",options:{hideLiquidity?:boolean;barrierRows?:BidAskWindow[];oiReport?:EtfReport;activity?:Omit<LevelActivityInput,'optionRows'|'expirations'|'optionFresh'>}={},now=Date.now()):string {
  if(currentPrice!=null&&(!Number.isFinite(currentPrice)||currentPrice<=0)){currentPrice=null;spotHtml='';}
  const s=displayStructure(horizon,structureFresh);
  const volumeAt=indexedLevelVolume(rows);
  const valid=!!s;
  const provisional=structureFresh?horizon?.structure.provisional:undefined;
  const quality=(low:number,high=low)=>levelQuality(horizon?.structure.levelQuality,low,high,!!provisional);
  if(horizon?.mode!=='DAY')flowFresh=false;
  const levels=new Map<number,{labels:string[];tone:string;scores:(number|null|undefined)[]}>();
  const add=(label:string,n:number|null|undefined,tone:string,score?:number|null)=>{if(n==null||strikeKey(n)==null)return;n=strikeKey(n)!/1000;const existing=levels.get(n);if(existing){existing.labels.push(label);existing.scores.push(score);}else levels.set(n,{labels:[label],tone,scores:[score]});};
  add('CALL WALL',s?.callWall?.strike,'call',s?.callWall?.score);add('IMÁN PRINCIPAL',s?.selected.M1,'magnet',s?.mainMagnetScore);add('PUT WALL',s?.putWall?.strike,'put',s?.putWall?.score);
  if(routeDesign&&!options.hideLiquidity)for(const route of s?.routes??[])add(`BARRERA DE LIQUIDEZ ${route.direction==='CALL'?'UP':'DOWN'}`,route.barrier,'barrier',route.barrierScore);
  const pull=(labels:string[])=>{
    const direction=labels.includes('CALL WALL')?'CALL':labels.includes('PUT WALL')?'PUT':null;
    if(!routeDesign||!direction)return '';
    const route=s?.routes.find(r=>r.direction===direction);
    const target=route?.pullTarget;
    const available=target!=null&&Number.isFinite(target)&&target>0;
    const oi=available?levelOpenInterest(options.oiReport,target,horizon?.expirations??[]):null;
    const pullQuant=available?quantitativeLevel(target,options.oiReport,options.barrierRows,horizon?.expirations??[],flowFresh):null;
    const pullDefense=available?levelDefense(target,outlook?.marketEvidence,outlook?.levelEvents,now):null;
    const caption=oi?oi.zeroOi?'CALL 0 · PUT 0 · sin OI en este nivel':`CALL ${oi.call} (${oi.callPct}%) · PUT ${oi.put} (${oi.putPct}%)`:'OI CALL / PUT: sin cobertura verificada';
    const local=available?quality(target):{provisional:!!provisional,detail:''};
    const strength=levelStrength(available?route?.score:undefined,false,local.provisional,direction+' PULL · pull_score del motor Rust; fórmula propia · '+local.detail+' · '+caption+' · '+(pullQuant?.detail??'')+' · '+(pullDefense?.label??'Defensa no verificable')+' · '+(pullDefense?.detail??''),{oi,caption});
    return `<div class="route-wall-pull"><span>${direction} PULL <b>${available?price(target):'—'}</b></span>${strength}</div>`;
  };
  const missingZones:string[]=[];
  const zones=['CALL','PUT'].flatMap(direction=>{
    const route=s?.routes.find(r=>r.direction===direction);
    const label=direction==='CALL'?'Vacío superior':'Vacío inferior';
    const low=route?.vacuumLow,high=route?.vacuumHigh;
    if(!route||low==null||high==null||!Number.isFinite(low)||!Number.isFinite(high)||low<=0||high<low){
      missingZones.push(`<article class="day-map-node vacuum"><div class="panel day-map-level"><span>${label}</span><strong>—</strong>${levelStrength(undefined,true)}<small>${valid?'Sin rango disponible en la estructura del día':'Esperando estructura vigente'}</small></div><div class="panel day-map-volume"><span>VOLUMEN EN LOS LÍMITES · 1 MINUTO</span><div><p><small>Límite inferior</small><b>—</b></p><p><small>Límite superior</small><b>—</b></p></div><small>Esperando límites y cobertura verificada</small></div></article>`);
      return [];
    }
    const local=quality(low,high);
    const status=local.provisional?'Cobertura local parcial':provisional?'Zona estimada · cadena parcial':route.vacuumStatus==='VACUUM'?'Clasificado como vacío por el modelo':route.vacuumStatus==='LOW-LIQUIDITY POCKET'?'Bolsillo de baja liquidez · sin confirmar':`Zona sin confirmar · ${route.vacuumStatus??'sin estado'}`;
    const boundaryDetail=[...new Set([low,high])].map(k=>`Límite ${k}: ${quantitativeLevel(k,options.oiReport,options.barrierRows,horizon?.expirations??[],flowFresh).detail}`).join(" ");
    const html=`<article class="day-map-node vacuum"><div class="panel day-map-level"><span>${label}</span><strong>${price(low)} — ${price(high)}</strong>${levelStrength(route.vacuumScore,true,local.provisional,label+' · '+local.detail)}<small title="${esc(boundaryDetail)}">${esc(status)}</small></div><div class="panel day-map-volume"><span>VOLUMEN EN LOS LÍMITES · 1 MINUTO</span><div><p><small>Inferior ${price(low)}</small><b>${formatVolume(volumeAt(low,horizon?.expirations??[],1,flowFresh))}</b></p><p><small>Superior ${price(high)}</small><b>${formatVolume(volumeAt(high,horizon?.expirations??[],1,flowFresh))}</b></p></div><small>Contratos CALL + PUT en cada strike; no es volumen total de la zona.</small></div></article>`;
    return [{position:(low+high)/2,html}];
  });
  const format=(n:number|null)=>n==null?'—':Intl.NumberFormat('es',{maximumFractionDigits:2}).format(n);
  const mapLevels:[number,{labels:string[];tone:string;scores:(number|null|undefined)[]}][]=[...levels];
  if(!options.hideLiquidity&&s?.magnet!=null&&Number.isFinite(s.magnet)&&s.magnet>0)mapLevels.push([s.magnet,{labels:['IMÁN DE LIQUIDEZ'],tone:'magnet',scores:[s.liquidityMagnetScore]}]);
  const activities=options.activity?readLevelActivities(mapLevels.map(([strike])=>strike),{...options.activity,optionRows:options.barrierRows,expirations:horizon?.expirations??[],optionFresh:flowFresh},now):undefined;
  const cards=mapLevels.sort(([a],[b])=>b-a).map(([strike,level])=>{
    const activity=activities?.get(strikeKey(strike)!);
    level.tone=level.labels.includes('CALL WALL')&&level.labels.includes('PUT WALL')?'mixed':level.labels.includes('CALL WALL')?'call':level.labels.includes('PUT WALL')?'put':level.labels.some(l=>l.startsWith('BARRERA DE LIQUIDEZ'))?'barrier':'magnet';
    const wallCard=level.labels.some(l=>l==='CALL WALL'||l==='PUT WALL');
    const oiCard=wallCard||level.labels.some(l=>l.startsWith("BARRERA DE LIQUIDEZ")||l.startsWith("IMÁN"));
    const oi=oiCard?levelOpenInterest(options.oiReport,strike,horizon?.expirations??[]):null;
    const bf=level.labels.some(l=>l.startsWith("BARRERA DE LIQUIDEZ"))?barrierFlow(options.barrierRows,strike,horizon?.expirations??[],flowFresh):null;
    const barrierCard=level.labels.some(l=>l.startsWith('BARRERA DE LIQUIDEZ'));
    const oiCaption=oi?oi.zeroOi?'OI CALL 0 · PUT 0 · sin OI en este nivel':wallCard?`CALL ${oi.call} (${oi.callPct}%) · PUT ${oi.put} (${oi.putPct}%)`:`OI CALL ${oi.callPct}% · PUT ${oi.putPct}%`:'OI CALL / PUT: sin cobertura verificada';
    const flowCaption=bf?`Flujo 5 min · CALL ${bf.callPct.toFixed(0)}% · PUT ${bf.putPct.toFixed(0)}%`:'Flujo CALL/PUT: sin cobertura';
    const barrierCaption=barrierCard?flowCaption:oiCard?oiCaption:undefined;
    const barrierAction=bf?.buyPct!=null&&bf.sellPct!=null&&Number.isFinite(bf.buyPct)&&Number.isFinite(bf.sellPct)?`Compra ${bf.buyPct.toFixed(0)}% · Venta ${bf.sellPct.toFixed(0)}% +`:'Compra/venta — · Lectura +';
    const oiDetail=oi?`Interés abierto ${oi.verification==='reference'?'cotejado con el mapa de OI':'del snapshot local, sin cotejo independiente'}: CALL ${oi.call} · PUT ${oi.put} · ${oi.contracts} series · vencimientos ${oi.expirations.join(", ")}. ${oi.zeroOi?'Ambos lados sin OI; porcentaje indefinido. ':''}${oi.verification==='reference'?'Población estándar de 100 acciones, sin entregables adicionales; ajustados excluidos. ':''}${oi.verification==='reference'&&!oi.scopeComplete?'Referencia global parcial; cifras de estas celdas verificadas. ':''}Fecha efectiva OI ${oi.oiDate??'no suministrada'}. Contratos abiertos, no compras/ventas ni inventario de dealers; mezcla CALL/PUT no confirma atracción del precio. `:"OI CALL / PUT: sin cobertura verificada; no se sustituye por flujo de operaciones. ";
    const strength=(score:number|null|undefined,label:string)=>levelStrength(score,false,quality(strike).provisional,label+' · '+quality(strike).detail,{oi:oiCard?oi:null,caption:wallCard?oiCaption:undefined,detail:oiDetail});
    const quant=quantitativeLevel(strike,options.oiReport,options.barrierRows,horizon?.expirations??[],flowFresh);
    const defense=levelDefense(strike,outlook?.marketEvidence,outlook?.levelEvents,now);
    const quantDetail=quant.detail+' '+defense.label+'. '+defense.detail;
    const barrierDetail=oiDetail+(bf?`5 min · strike exacto · muestra suscrita · CALL ${bf.call} / PUT ${bf.put} contratos · ASK (compra inferida) ${bf.buy} / BID (venta inferida) ${bf.sell} · sin clasificar ${bf.unknown} · cobertura clasificada ${bf.coverage.toFixed(1)}%. No indica intención ni apertura/cierre; no es porcentaje de fuerza estructural.`:"")+quantDetail;
    const v1=volumeAt(strike,horizon?.expirations??[],1,flowFresh),v5=volumeAt(strike,horizon?.expirations??[],5,flowFresh);
    return {position:strike,html:`<article class="day-map-node ${level.tone}${oiCard?" level-oi-card":""}${activity?' level-activity-card':''}"><div class="panel day-map-level"${activity?' '+activityStyle(activity):''}><span class="day-map-symbol" aria-hidden="true">${level.tone==='call'?'↑':level.tone==='put'?'↓':'◎'}</span><span>${esc(level.labels.join(' · '))}</span><strong>${price(strike)}</strong>${activity?activityChips(activity):''}${level.scores.map((score,i)=>strength(score,level.labels[i]+" · "+barrierDetail)).join('')}${pull(level.labels)}${directionLevelTag(linkedLevels,strike)}${mapLevelReading(level.labels,strike,outlook,barrierCaption,barrierCard?barrierAction:wallCard?defense.label:oiCard?`${barrierCaption} +`:barrierAction,barrierDetail)}<small class="atr-inline-summary" title="${esc(currentPrice==null?'Sin precio reciente':[atrDistance(atr,currentPrice,strike),dailyDistance(atr,currentPrice,strike)].filter(Boolean).join(' · '))}">${currentPrice==null?'Sin precio reciente':`Delta nivel−precio: ${price(strike-currentPrice)} USD${atrDistance(atr,currentPrice,strike)?" · "+atrDistance(atr,currentPrice,strike):""}`}</small></div><div class="panel day-map-volume"><span>VOLUMEN DE OPCIONES · STRIKE EXACTO</span><div><p><small>Último minuto</small><b>${format(v1)}</b></p><p><small>Últimos 5 minutos</small><b>${format(v5)}</b></p></div><small>${v1==null||v5==null?'Ventana sin cobertura completa: —':'Ventanas observadas'} · contratos CALL + PUT</small></div></article>`};
  }).concat(zones).sort((a,b)=>b.position-a.position).map(n=>n.html).join('');
  const placeholder=(label:string,tone:string)=>`<article class="day-map-node ${tone}"><div class="panel day-map-level"><span>${label}</span><strong>—</strong>${levelStrength(undefined,tone==='vacuum')}${tone==='vacuum'?'':mapLevelReading([label],undefined,outlook)}<small>Esperando nivel vigente</small></div><div class="panel day-map-volume"><span>VOLUMEN EN EL NIVEL</span><div><p><small>1 minuto</small><b>—</b></p><p><small>5 minutos</small><b>—</b></p></div><small>Sin cobertura verificada</small></div></article>`;
  const displayCards=cards
    ? cards+missingZones.join('')+(!options.hideLiquidity&&s?.magnet==null?placeholder('IMÁN DE LIQUIDEZ','magnet'):'')
    : placeholder('CALL WALL','call')+(missingZones[0]??'')+placeholder('IMÁN PRINCIPAL','magnet')+(options.hideLiquidity?'':placeholder('IMÁN DE LIQUIDEZ','magnet'))+(missingZones[1]??'')+placeholder('PUT WALL','put');
  let routeRow=0;
  const positionedCards=routeDesign?displayCards.replace(/<article class="day-map-node /g,()=>`<article style="grid-row:${++routeRow}" class="day-map-node `):displayCards;
  const spot=`<div class="day-map-spot" aria-label="Precio del activo seleccionado">${spotHtml||`<strong>${currentPrice==null?'—':price(currentPrice)}</strong>`}</div>`;
  const header=routeDesign?`<header class="reading-toolbar direction-header"><div class="day-map-title-price"><h2>Mapa de niveles</h2>${spot}<div class="map-header-context">${linkedLevels?directionLevelsPanel(linkedLevels):""}${headerContext}</div></div>${directionHtml}</header>`:`<header class="reading-toolbar"><div><h2>Mapa de niveles</h2>${horizon?.mode==='NEXT_SESSION'?`<small>Próximo vencimiento · ${esc(horizon.expirations.join(', '))} · </small>`:''}${provisional?`<small>Cobertura de cadena · ${provisional.coveredContracts}/${provisional.totalContracts} contratos · sin señal confirmada</small>`:''}</div><div class="day-map-price-row">${spot}${directionHtml}</div></header>`;
  const health=mapDataQuality([
   {name:'Estructura',kind:'derived',available:valid,asOf:horizon?.observedMs??null,source:horizon?.referenceSource??'Fuente no identificada',method:'Modelo estructural Snipers; versión de fórmula no aportada'},
   {name:'OI',kind:'observed',available:!!options.oiReport?.enrichment,asOf:options.oiReport?.at??null,source:options.oiReport?.heatmap?'Cadena con referencia independiente':'Snapshot de opciones',method:`Vencimientos del horizonte; fecha efectiva OI ${options.oiReport?.enrichment?.oiReferenceDate??'no suministrada'}`},
   {name:'Flujo 1m/5m',kind:'observed',available:flowFresh,asOf:null,source:'Muestra suscrita',method:'Sin timestamp común por ventana en ContractWindow'},
   {name:'Precio',kind:'observed',available:currentPrice!=null,asOf:null,source:'Precio recibido por la interfaz',method:'Hora del trade no aportada a dayMap'},
   {name:'ATR',kind:'derived',available:atr?.value!=null,asOf:atr?.endMs??null,source:'Velas cerradas',method:'ATR14 SMA de true ranges'},
   {name:'Contexto',kind:'interpretation',available:!!outlook?.marketEvidence,asOf:outlook?.marketEvidence?.at??null,source:'Control intradía',method:'Interpretación compartida; no certifica una señal'}
  ]);
  const activityLegend=options.activity?`<div class="level-activity-legend" aria-label="Colores de actividad"><span class="activity-acceptance-key">Aceptación AMT</span><span class="activity-stock-key">Ventas acciones · 60 s</span><span class="activity-options-key">Ventas opciones · 5 min</span><small>${options.activity.captured?'En la captura guardada':'Muestra observada'} · máximos entre cuadros con datos, por fuente</small></div>`:'';
  return `<section class="day-map day-map-constellation">${header}${!routeDesign&&linkedLevels?directionLevelsPanel(linkedLevels):""}${activityLegend}<div class="day-map-track" aria-label="Niveles de mayor a menor precio">${positionedCards}</div>${health.html}</section>`;
}
