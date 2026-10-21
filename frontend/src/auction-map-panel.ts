import {auctionPopulationDetail,type AuctionPopulationComparison} from './auction-population-detail';
import {auctionDisplayMemory,auctionDisplayDay} from './auction-display-memory';
import {escapeHtml as esc} from './model';
import './auction-map-panel.css';
import {auctionVolumeProfile} from './auction-volume-profile';
export interface AuctionBin {low:number;high:number;volume:number;brackets:number;volumeRatio:number;timeRatio:number;kind:string}
export interface AuctionRoute {direction:number;destinationLow:number;destinationHigh:number;invalidationLevel:number;invalidation:string;trigger:string;probability:null}
export interface RejectionReading {version:string;symbol:string;status:string;score:number|null;distance:number|null;permanence:number|null;participation:number|null;level:number;windowStartMs:number;windowEndMs:number;calibrated:boolean;probability:null}
export interface AuctionReading {profilePopulations?:AuctionPopulationComparison;sessionHistory?:{status:string;fromOpen:boolean;throughMs:number|null;pages:number;rows:number;liveProvisional:boolean};rejectionUpper?:RejectionReading|null;rejectionLower?:RejectionReading|null;observedSinceMs?:number;scope?:string;version:string;symbol:string;asOfMs:number;provisional:boolean;acceptedTrades:number;totalVolume:number;pocLow:number;pocHigh:number;val:number;vah:number;bins:AuctionBin[];reaction:string;routes:AuctionRoute[];calibrated:boolean}
const number=(v:number)=>v.toLocaleString('en-US',{minimumFractionDigits:2,maximumFractionDigits:2,useGrouping:false});
const zone=(lo:number,hi:number)=>`${number(lo)}–${number(hi)}`;
export function levelIntensity(bin:AuctionBin|undefined){
 if(!bin||bin.volume<=0||!Number.isInteger(bin.brackets)||bin.brackets<1||![bin.volumeRatio,bin.timeRatio].every(v=>Number.isFinite(v)&&v>=0&&v<=1))return;
 const score=Math.sqrt(bin.volumeRatio*bin.timeRatio);
 return {score,label:score>=.85?'Alta':score>=.65?'Media':'Baja',fill:Math.round(10+score*38),edge:Math.round(25+score*75)};
}
/** Visual acceptance only: geometric mean prevents either volume or time dominating alone. */
export function acceptanceIntensity(bin:AuctionBin|undefined){
 if(!bin||bin.kind!=='acceptance'||!Number.isInteger(bin.brackets)||bin.brackets<2
  ||![bin.volumeRatio,bin.timeRatio].every(v=>Number.isFinite(v)&&v>=.5&&v<=1))return;
 return levelIntensity(bin);
}
export function rejectionEvidence(r:RejectionReading|null|undefined,symbol:string,now:number){
 if(!r||r.version!=='auction-rejection-index-v1'||r.symbol!==symbol||r.status!=='observed'||r.calibrated||r.probability!==null||r.score==null||![r.score,r.level,r.windowStartMs,r.windowEndMs,r.distance,r.permanence,r.participation].every(v=>v!=null&&Number.isFinite(v))||r.level<=0||r.windowStartMs>=r.windowEndMs||r.windowEndMs>now||now-r.windowEndMs>300000||r.score<0||r.score>100||[r.distance!,r.permanence!,r.participation!].some(v=>v<0||v>1)||Math.abs(r.score-100*Math.cbrt(r.distance!*r.permanence!*r.participation!))>1e-4)return;
 return r;
}
/** No demo prices, option-strike substitutions or inferred probabilities in the live UI. */
export function auctionMapPanel(symbol:string,active:boolean,now:number,reading?:AuctionReading,spot:number|null=null,reference?:import('./auction-history').AuctionReference,captured=false):string {
 const capturedCandidate=!!reading&&captured&&reading.symbol===symbol&&Number.isFinite(reading.asOfMs)&&reading.asOfMs>0&&reading.asOfMs<=now&&reading.version==='auction-map-ticks-v1'&&auctionDisplayDay(reading.asOfMs)===auctionDisplayDay(now);
 const liveCandidate=!!reading&&!captured&&active&&reading.symbol===symbol&&reading.asOfMs<=now&&now-reading.asOfMs<=15000&&reading.version==='auction-map-ticks-v1'&&auctionDisplayDay(reading.asOfMs)===auctionDisplayDay(now);
 const saved=captured?undefined:auctionDisplayMemory.read(symbol,now);
 const retained=!liveCandidate&&saved?{...saved,reaction:'pending',routes:[],rejectionUpper:null,rejectionLower:null}:undefined;
 const historical=!liveCandidate&&!retained&&!!reference&&reference.symbol===symbol&&reference.reading.symbol===symbol&&reference.reading.version==='auction-map-bars-reference-v1'&&reference.reading.asOfMs<=now&&now-reference.reading.asOfMs<=14*86400000&&Number.isFinite(reference.close)&&reference.close>0;
 const r=liveCandidate||capturedCandidate?reading:retained??(historical?reference!.reading:reading);
 const valid=!!r&&(historical||!!retained||capturedCandidate||(active&&r.version==='auction-map-ticks-v1'&&Number.isFinite(r.asOfMs)&&r.asOfMs<=now&&now-r.asOfMs<=15000))&&r.symbol===symbol
  &&[r.pocLow,r.pocHigh,r.val,r.vah,r.totalVolume].every(v=>Number.isFinite(v)&&v>0)&&r.val<r.vah&&r.pocLow<r.pocHigh&&(historical||r.acceptedTrades>0)
  &&Array.isArray(r.bins)&&r.bins.every(b=>[b.low,b.high,b.volume,b.brackets,b.volumeRatio,b.timeRatio].every(Number.isFinite)&&b.low>0&&b.high>b.low&&b.volume>=0)
  &&Array.isArray(r.routes)&&r.routes.every(t=>[t.direction,t.destinationLow,t.destinationHigh,t.invalidationLevel].every(Number.isFinite)&&[1,-1].includes(t.direction)&&t.destinationLow>0&&t.destinationHigh>t.destinationLow&&t.invalidationLevel>0&&t.probability===null);
 if(valid&&liveCandidate)auctionDisplayMemory.remember(r!);
 const initial=valid&&!historical&&Math.max(...r!.bins.map(b=>b.brackets))<2;
 const current=historical&&valid?reference!.close:spot!=null&&Number.isFinite(spot)&&spot>0?spot:null;
 const priceRef=current??(valid?(r!.pocLow+r!.pocHigh)/2:null);
 const upper=valid&&priceRef!=null?r!.bins.filter(b=>b.kind==='acceptance'&&b.low>priceRef).sort((a,b)=>a.low-b.low)[0]:undefined;
 const lower=valid&&priceRef!=null?r!.bins.filter(b=>b.kind==='acceptance'&&b.high<priceRef).sort((a,b)=>b.high-a.high)[0]:undefined;
 const rejectionUpper=valid&&!historical&&!retained?rejectionEvidence(r!.rejectionUpper,symbol,now):undefined,rejectionLower=valid&&!historical?rejectionEvidence(r!.rejectionLower,symbol,now):undefined;
 const cards=[
  {key:'upper',label:'Aceptación superior',tone:'acceptance',value:upper?zone(upper.low,upper.high):null,detail:upper?'Zona con volumen y presencia temporal observados.':'Esperando una zona superior con volumen y permanencia suficientes.'},
  {key:'upper-rejection',label:'Rechazo superior',tone:'rejection',value:rejectionUpper?number(rejectionUpper.level):valid&&r!.reaction==='upper-rejection'?number(r!.vah):null,detail:'Necesita visita al borde y dos cierres de regreso; poco volumen por sí solo no demuestra rechazo.'},
  {key:'vah',label:'VAH · Borde superior',tone:'value',value:valid?number(r!.vah):null,detail:'Borde superior del área de valor del perfil observado.'},
  {key:'poc',label:'POC · Máximo volumen',tone:'poc',value:valid?zone(r!.pocLow,r!.pocHigh):null,detail:valid&&r!.profilePopulations?auctionPopulationDetail(symbol,r!.asOfMs,r!.profilePopulations):'Intervalo con mayor volumen negociado; no es una garantía de atracción del precio.'},
  {key:'val',label:'VAL · Borde inferior',tone:'value',value:valid?number(r!.val):null,detail:'Borde inferior del área de valor del perfil observado.'},
  {key:'lower-rejection',label:'Rechazo inferior',tone:'rejection',value:rejectionLower?number(rejectionLower.level):valid&&r!.reaction==='lower-rejection'?number(r!.val):null,detail:'Necesita visita al borde y dos cierres de regreso; no equivale a soporte confirmado.'},
  {key:'lower',label:'Aceptación inferior',tone:'acceptance',value:lower?zone(lower.low,lower.high):null,detail:lower?'Zona con volumen y presencia temporal observados.':'Esperando una zona inferior con volumen y permanencia suficientes.'},
 ];
 const dateLabel=historical&&valid?new Date(r!.asOfMs).toLocaleDateString('es',{timeZone:'America/New_York',weekday:'long',day:'2-digit',month:'2-digit'}):'';
 const sessionHistory=r?.sessionHistory;
 const sessionStatus=sessionHistory?.fromOpen&&sessionHistory.throughMs!=null?`Perfil RTH desde apertura · REST hasta ${new Date(sessionHistory.throughMs).toLocaleTimeString('es',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit'})} ET · en vivo provisional`:sessionHistory?.status==='loading'?`Recuperando sesión RTH · ${sessionHistory.pages} páginas · perfil parcial`:sessionHistory?.status&&sessionHistory.status!=='ready'?`Perfil parcial · ${sessionHistory.status}`:null;
 const status=!valid?'Pendiente de perfil del subyacente':capturedCandidate?`Perfil en captura · lectura ${new Date(r!.asOfMs).toLocaleTimeString('es',{timeZone:'America/New_York'})} ET`:retained?`Perfil del día guardado · ${active?'sin vigencia':'desconectado'} · última lectura ${new Date(r!.asOfMs).toLocaleTimeString('es',{timeZone:'America/New_York'})} ET`:historical?`Referencia histórica estimada · ${dateLabel}`:sessionStatus??(r!.provisional?`Perfil observado · parcial${r!.observedSinceMs!=null?' · desde '+new Date(r!.observedSinceMs!).toLocaleTimeString('es',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit'}):''}`:'Perfil observado');
 const displayCards=[cards[1],cards[2],cards[3],cards[0],cards[6],cards[4],cards[5]];
 const cardMarkup=displayCards.map((c,i)=>{
  const boundary=valid?(c.key==='vah'?r!.vah:c.key==='val'?r!.val:null):null;
  const bin=c.key==='upper'?upper:c.key==='lower'?lower:c.key==='poc'&&valid?r!.bins.find(b=>b.low===r!.pocLow&&b.high===r!.pocHigh):boundary!=null?r!.bins.find(b=>c.key==='vah'?b.low<boundary&&b.high>=boundary:b.low<=boundary&&b.high>boundary):undefined;
  const rejection=c.key==='upper-rejection'?rejectionUpper:c.key==='lower-rejection'?rejectionLower:undefined;
  const confirmedRejection=c.tone==='rejection'&&c.value!=null;
  const intensity=c.value==null?undefined:rejection?{score:rejection.score!/100,label:'Experimental',fill:Math.round(10+rejection.score!*.38),edge:Math.round(25+rejection.score!*.75)}:confirmedRejection?{score:1,label:'Confirmado',fill:48,edge:100}:c.tone==='acceptance'?acceptanceIntensity(bin):levelIntensity(bin);
  const style=intensity?` style="--level-fill:${intensity.fill}%;--level-edge:${intensity.edge}%" data-level-strength="${intensity.score.toFixed(4)}"${c.tone==='acceptance'?` data-acceptance-strength="${intensity.score.toFixed(4)}"`:''}`:'';
  const percent=rejection?Math.round(rejection.score!):c.tone!=='rejection'&&intensity?Math.round(intensity.score*100):undefined;
  const concept=c.tone==='rejection'?'Fuerza de rechazo':c.tone==='acceptance'?'Aceptación':'Participación';
  const label=c.value==null?'Pendiente de datos':retained&&percent!=null?`${concept} ${percent}% · guardada`:initial&&c.tone!=='rejection'&&percent!=null?`Participación relativa ${percent}% · inicial`:percent!=null?`${concept} ${percent}%${historical?' · estimada':rejection?' · experimental':` · ${intensity!.label}`}`:confirmedRejection?'Rechazo observado · % pendiente':historical?'Estimado con velas':'Participación pendiente';
  const meter=percent==null?'':`<span class="auction-acceptance-meter" role="meter" aria-label="Índice de ${concept.toLowerCase()}${historical?' estimada':''}" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${percent}" aria-valuetext="${percent}%; índice descriptivo, no probabilidad de llegada"><i style="width:${percent}%"></i></span>`;
  const detail=rejection?` Rechazo medido en cinco minutos: distancia ${Math.round(rejection.distance!*100)}%, permanencia ${Math.round(rejection.permanence!*100)}%, volumen ${Math.round(rejection.participation!*100)}%. Índice experimental; no probabilidad. Ventana terminada ${new Date(rejection.windowEndMs).toLocaleTimeString('es',{timeZone:'America/New_York'})} ET.`:intensity?confirmedRejection?' El rojo se intensifica cuando se observa el rechazo; no representa una puntuación de confianza.':` Importancia ${historical?'estimada':'observada'} ${intensity.label.toLowerCase()}: combina volumen relativo y presencia temporal del mismo perfil. La intensidad del color no mide probabilidad de llegada.`:'';
  return `${i===4?`<div class="auction-map-price"><span>${historical&&valid?'CIERRE DE LA SESIÓN':'PRECIO ACTUAL'}</span><b>${current==null?'—':number(current)}</b></div>`:''}<details class="auction-map-node ${c.tone}${c.value==null?' pending':''}"${style} data-horizon-details="auction-${c.key}"><summary><span>${esc(c.label)}</span><strong>${c.value??'—'}</strong><small>${esc(label)}</small>${meter}</summary><p>${esc(c.detail+detail+(retained?' Última lectura guardada del día; sin confirmación actual.':''))}</p></details>`;
 }).join('');
 return `<section class="structural-map-preview auction-map-panel" aria-label="Mapa de aceptación y rechazo"><div class="section-head"><strong>MAPA DE ACEPTACIÓN Y RECHAZO</strong><span>${esc(symbol)} · AMT</span></div><div class="structural-price"><strong>${esc(status)}</strong>${current!=null?`<span>${historical?'Cierre':'Precio'} ${number(current)}</span>`:''}</div><div class="auction-map-visual">${auctionVolumeProfile(valid?r:undefined,current,upper,lower)}<div class="auction-map-levels"><div class="auction-levels-heading">NIVELES · ACEPTACIÓN</div><div class="auction-map-track">${cardMarkup}</div></div></div><details class="legend" data-horizon-details="auction-reading"><summary>Lectura de subasta y posibles destinos</summary>${valid&&r!.routes.length?r!.routes.map(t=>`<p>Posible destino ${zone(t.destinationLow,t.destinationHigh)} · ${esc(t.trigger)}. ${esc(t.invalidation)}: ${number(t.invalidationLevel)}.</p>`).join(''):'<p>Esperando una ruta con aceptación observada, reacción y nivel de invalidación. No se inventa un destino fuera del perfil disponible.</p>'}${retained?'<p>Se conserva el perfil observado del día. La publicación no está vigente: rechazos y rutas actuales quedan pendientes; los porcentajes describen la última muestra guardada.</p>':''}${historical&&valid?'<p>Estimación OHLCV: el volumen de cada vela se reparte entre sus intervalos de precio. No es un perfil tick exacto ni una señal en vivo. Las zonas verdes son candidatas estimadas; no se confirma rechazo con esta referencia.</p>':''}<p>Verde: aceptación · rojo: rechazo · cian: área de valor · morado: POC. Datos del subyacente; el OI de opciones no sustituye el volumen por precio. Sin porcentajes de llegada calibrados.</p></details></section>`;
}
export function quickMapStack(structural:string,auction:string):string {
 return `<aside class="intraday-control control-structured auction-map-stack" aria-label="Mapas del activo">${auction}${structural}</aside>`;
}
