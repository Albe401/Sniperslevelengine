import {escapeHtml as esc} from './model';
import type {AuctionBin,AuctionReading} from './auction-map-panel';
import {auctionProfileRange} from './auction-profile-navigation';
/** Display aggregation only: volume conserved, no inferred prices or rejection scores. */
export function profileGroups(bins:AuctionBin[],limit=80){
 const sorted=bins.filter(b=>b.volume>0).sort((a,b)=>a.low-b.low),size=Math.max(1,Math.ceil(sorted.length/Math.max(1,limit))),groups:{low:number;high:number;volume:number;acceptance:boolean}[]=[];
 let part:AuctionBin[]=[];
 const flush=()=>{if(part.length){groups.push({low:part[0].low,high:part.at(-1)!.high,volume:part.reduce((n,b)=>n+b.volume,0),acceptance:part.every(b=>b.kind==='acceptance'&&b.volumeRatio>=.5&&b.timeRatio>=.5&&b.brackets>=2)});part=[];}};
 for(const bin of sorted){if(part.length&&(part.length>=size||Math.abs(part.at(-1)!.high-bin.low)>1e-8))flush();part.push(bin);}flush();
 return groups;
}
/** Only supplied conditional routes ahead of price and backed by an observed acceptance bin. */
export function profileDestinations(reading:AuctionReading,current:number|null){
 if(current==null||!Number.isFinite(current)||!Array.isArray(reading.routes))return [];
 const candidates=reading.routes.filter(t=>[t.direction,t.destinationLow,t.destinationHigh].every(Number.isFinite)&&[1,-1].includes(t.direction)&&t.destinationLow>0&&t.destinationHigh>t.destinationLow&&t.probability===null
  &&(t.direction===1?t.destinationLow>current:t.destinationHigh<current)
  &&reading.bins.some(b=>b.kind==='acceptance'&&b.volumeRatio>=.5&&b.timeRatio>=.5&&b.brackets>=2&&b.low<=t.destinationLow&&b.high>=t.destinationHigh))
  .sort((a,b)=>Math.abs((a.destinationLow+a.destinationHigh)/2-current)-Math.abs((b.destinationLow+b.destinationHigh)/2-current));
 return candidates.filter((t,i)=>candidates.findIndex(v=>v.direction===t.direction)===i);
}
export function auctionVolumeProfile(reading:AuctionReading|undefined,current:number|null,upper?:AuctionBin,lower?:AuctionBin){
 if(!reading?.bins.length)return '<section class="auction-volume-profile" aria-label="Perfil de volumen"><header><strong>PERFIL DE VOLUMEN</strong><span>Precio ($)</span></header><div class="auction-profile-empty">Pendiente de volumen por precio del subyacente</div></section>';
 const groups=profileGroups(reading.bins);
 const observedLo=Math.min(...groups.map(b=>b.low)),observedHi=Math.max(...groups.map(b=>b.high));
 const binSize=Math.max(...groups.map(b=>b.high-b.low));
 // Stable thin rows across assets, including sparse connection samples.
 const price= current!=null&&Number.isFinite(current)&&current>0?current:(observedLo+observedHi)/2;
 const span=Math.max(observedHi-observedLo,binSize*80),center=(observedLo+observedHi)/2;
 const fit={lo:Math.min(center-span/2,price),hi:Math.max(center+span/2,price)};
 const viewKey=`${reading.symbol}:${reading.version}:${new Date(reading.asOfMs).toLocaleDateString('en-CA',{timeZone:'America/New_York'})}`;
 const {lo,hi}=auctionProfileRange(viewKey,fit,binSize*80),range=hi-lo,max=Math.max(...groups.map(b=>b.volume));
 const initial=reading.version==='auction-map-ticks-v1'&&Math.max(...reading.bins.map(b=>b.brackets))<2;
 if(!Number.isFinite(range)||range<=0||!Number.isFinite(max)||max<=0)return '<section class="auction-volume-profile"><div class="auction-profile-empty">Perfil no disponible</div></section>';
 const y=(price:number)=>{const value=100*(hi-price)/range;return Math.abs(value)<1e-8?0:value;},n=(price:number)=>price.toFixed(2);
 const overlap=(a:{low:number;high:number},b:{low:number;high:number}|undefined)=>!!b&&a.low<b.high&&a.high>b.low;
 const targets=profileDestinations(reading,current);
 const currentBin=current==null?undefined:reading.bins.find(b=>current>=b.low&&(current<b.high||current===hi&&b.high===hi));
 const bars=groups.map(b=>{const poc=overlap(b,{low:reading.pocLow,high:reading.pocHigh}),u=overlap(b,upper),l=overlap(b,lower),atPrice=overlap(b,currentBin),target=targets.some(t=>overlap(b,{low:t.destinationLow,high:t.destinationHigh})),label=`${n(b.low)}–${n(b.high)} · volumen ${b.volume.toLocaleString('en-US')} acciones${atPrice?' · nivel del precio':''}${target?' · destino condicionado':''}${poc?' · POC':b.acceptance?' · aceptación':''}`;
  return `<div class="auction-profile-bin${poc?' is-poc':b.acceptance?' is-acceptance':''}${u?' zone-upper':''}${l?' zone-lower':''}${atPrice?' is-current':''}${target?' is-destination':''}" style="top:${y(b.high).toFixed(4)}%;height:${(100*(b.high-b.low)/range).toFixed(4)}%" title="${esc(label)}" aria-label="${esc(label)}"><i style="width:${(100*b.volume/max).toFixed(3)}%"></i></div>`;
 }).join('');
 const ticks=[hi,hi-range*.25,hi-range*.5,hi-range*.75,lo].map((p,i)=>`<span style="top:${i*25}%">${n(p)}</span>`).join('');
 const line=(price:number,name:string,tone:string)=>Number.isFinite(price)?`<div class="auction-profile-line ${tone}" style="top:${y(price).toFixed(4)}%" title="${esc(name)} ${n(price)}"><span>${esc(name)}</span></div>`:'';
 const arrows=current==null||current<lo||current>hi?'':targets.map((t,i)=>{const destination=(t.destinationLow+t.destinationHigh)/2,from=y(current),to=y(destination);return `<div class="auction-profile-path ${t.direction===1?'up':'down'}" style="top:${Math.min(from,to).toFixed(4)}%;height:${Math.abs(to-from).toFixed(4)}%;right:${5+i*7}px" aria-label="Posible recorrido condicionado hacia ${n(destination)}"></div>`;}).join('');
 const destinationLabels=targets.length?targets.map(t=>`<span>Destino condicionado ${t.direction===1?'↑':'↓'} <b>${n(t.destinationLow)}–${n(t.destinationHigh)}</b></span>`).join(''):'<span>Destino pendiente · sin ruta válida</span>';
 const navigation=`<div class="auction-profile-navigation"><button type="button" data-auction-pan="0.15" aria-label="Ver precios superiores">↑</button><button type="button" data-auction-pan="reset">Ajustar</button><button type="button" data-auction-pan="-0.15" aria-label="Ver precios inferiores">↓</button><small>Arrastra o usa la rueda ↑↓</small></div>`;
 return `<section class="auction-volume-profile" aria-label="Perfil de volumen por precio"><header><strong>PERFIL DE VOLUMEN</strong><span>Precio ($)</span></header>${navigation}${initial?'<small class="auction-profile-quality">Muestra inicial · presencia temporal insuficiente. Volumen relativo; no fuerza confirmada.</small>':''}<div class="auction-profile-plot" style="min-height:0;flex:1" data-auction-profile-key="${esc(viewKey)}" data-fit-lo="${fit.lo}" data-fit-hi="${fit.hi}" data-view-lo="${lo}" data-view-hi="${hi}" tabindex="0" role="group" aria-label="Perfil AMT desplazable: arrastra, usa la rueda o las flechas para ver otros precios"><div class="auction-profile-bars">${bars}${line(reading.vah,'VAH','value upper')}${line(reading.val,'VAL','value lower')}${current!=null?line(current,'Precio','reference'):''}${arrows}</div><div class="auction-profile-axis">${ticks}</div></div><div class="auction-profile-scale"><span>0</span><span>Volumen relativo</span><span>Máximo</span></div><div class="auction-profile-legend"><span>Volumen</span><span class="acceptance">Aceptación</span><span class="poc">POC</span><span class="current">Nivel del precio</span><span class="destination">Destino</span></div><div class="auction-profile-destinations">${destinationLabels}</div>${groups.length<reading.bins.length?'<small>Intervalos agrupados para la vista; volumen conservado.</small>':''}</section>`;
}
