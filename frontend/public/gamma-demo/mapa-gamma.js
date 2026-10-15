import {createChart,CandlestickSeries,HistogramSeries,LineSeries,ColorType,CrosshairMode,aggregate} from './chart-renderer/tradingview-runtime.js';
import {transitionReading} from './transition-reading.mjs';
import {levelMarkers,routeBarriers,directionalPulls} from './level-markers.mjs';
const $=id=>document.getElementById(id);
const strikes=Array.from({length:31},(_,i)=>725+i);
const state={scenario:'fall',step:8,selected:740,zoom:1,center:null,playing:false,minutes:2};
const curve=(k,c,w)=>Math.exp(-(((k-c)/w)**2));
const money=n=>`${n<0?'−':''}$${Math.abs(n).toFixed(1)}M`;
const points=n=>`${n>0?'+':''}${n.toFixed(1)} pts`;
// Deterministic fixtures: each moment changes gamma/activity, never intraday OI.
function demoCandles(path){
 const anchors=[738,741,745,748,743,749,740,733,736,path[0]];
 const history=Array.from({length:30},(_,i)=>{const q=i/29*(anchors.length-1),a=Math.min(anchors.length-2,Math.floor(q));return anchors[a]+(anchors[a+1]-anchors[a])*(q-a);});
 const closes=[...history,...path];let pv=0,volume=0;
 return closes.map((close,i)=>{const open=i?closes[i-1]:close-.1,high=Math.max(open,close)+.12,low=Math.min(open,close)-.12,v=Math.round(12000+Math.abs(close-open)*80000);pv+=(high+low+close)/3*v;volume+=v;return {timestampMs:Date.UTC(2026,8,30,13,30+i*2),open,close,high,low,volume:v,vwap:pv/volume};});
}
function fixture(s,t){
 const prices={fall:[741.4,741.8,742.1,742.4,742.0,741.5,741.1,740.6,739.8,739.2,738.6,737.9,737.2],rebound:[738.3,738.0,737.8,738.1,738.4,738.9,739.4,739.8,740.4,741.0,741.5,742.0,742.4],absorb:[740.2,740.0,739.9,739.8,739.9,739.8,739.9,739.8,739.9,739.8,739.9,740.0,740.1]};
 const path=prices[s],spot=path[t],flow=s==='rebound'?0.68:s==='fall'?-0.72:-0.62;
 const rows=strikes.map(k=>{
  const oiCall=Math.round(1800+42000*curve(k,745,3)+27000*curve(k,740,2));
  const oiPut=Math.round(2200+61000*curve(k,730,3)+33000*curve(k,738,2));
  const call=(8+160*curve(k,745,2.1)+96*curve(k,740,1.6))*(1+0.012*t*(s==='rebound'?1:-0.3));
  const put=(6+210*curve(k,730,2)+90*curve(k,735,1.8)+55*curve(k,740,1.6))*(1+0.018*t*(s==='fall'?1:0.15));
  const activity=curve(k,s==='fall'?738:s==='rebound'?742:740,1.8);
  const premium=0.18+activity*(0.5+t*0.2)+curve(k,745,2)*0.35;
  const volume=Math.round(120+activity*(700+t*230)+curve(k,730,2)*200);
  // Reverse the fixture dollar GEX into a consistent synthetic contract gamma.
  const gammaCall=call*1e6/(oiCall*100*spot*spot*.01),gammaPut=put*1e6/(oiPut*100*spot*spot*.01);
  return {k,call,put,net:call-put,oiCall,oiPut,premium,volume,flow,gammaCall,gammaPut};
 });
 const maxima={gamma:Math.max(...rows.map(r=>r.call+r.put)),premium:Math.max(...rows.map(r=>r.premium)),volume:Math.max(...rows.map(r=>r.volume)),oi:Math.max(...rows.map(r=>r.oiCall+r.oiPut))};
 for(const r of rows)r.score=100*(.35*(r.call+r.put)/maxima.gamma+.25*r.premium/maxima.premium+.15*r.volume/maxima.volume+.15*(r.oiCall+r.oiPut)/maxima.oi+.1*Math.exp(-Math.abs(r.k-spot)/3));
 const visible=path.slice(0,t+1);
 return {spot,rows,path:visible,flow,vwap:demoCandles(visible).at(-1).vwap};
}
function gammaVoids(rows,spot){
 const threshold=Math.max(...rows.map(r=>r.call+r.put))*.40;
 const groups=[];let run=[];
 const flush=()=>{if(run.length>=2)groups.push({low:run[0].k-.5,high:run.at(-1).k+.5,count:run.length});run=[];};
 for(const r of rows.slice().sort((a,b)=>a.k-b.k)){if(r.call+r.put<=threshold){if(run.length&&r.k-run.at(-1).k!==1)flush();run.push(r);}else flush();}flush();
 return {upper:groups.filter(g=>g.low>spot).sort((a,b)=>a.low-b.low)[0]??null,lower:groups.filter(g=>g.high<spot).sort((a,b)=>b.high-a.high)[0]??null,threshold};
}
function model(){
 const now=fixture(state.scenario,state.step),previous=fixture(state.scenario,Math.max(0,state.step-1));
 now.rows.forEach((r,i)=>{r.delta=state.step===0?null:r.score-previous.rows[i].score;r.gexChange=state.step===0?null:r.call+r.put-previous.rows[i].call-previous.rows[i].put;});
 const changes=now.rows.slice().sort((a,b)=>Math.abs(b.delta??0)-Math.abs(a.delta??0));
 const move=now.spot-now.path[Math.max(0,now.path.length-4)];
 const above=now.path.length>=2&&now.path.slice(-2).every(p=>p>now.vwap+.05);
 const below=now.path.length>=2&&now.path.slice(-2).every(p=>p<now.vwap-.05);
 const flat=now.path.length>=4&&Math.max(...now.path.slice(-4))-Math.min(...now.path.slice(-4))<.3;
 const direction=move>.3&&now.flow>.2&&above?'up':move<-.3&&now.flow<-.2&&below?'down':'wait';
 return {...now,changes,direction,flat,voids:gammaVoids(now.rows,now.spot),magnet:state.scenario==='rebound'?745:740,up:now.spot<740?740:745,down:now.spot>738.6?738.6:735};
}
function render(){
 const m=model();const signal=m.direction==='up'?'#43dba6':m.direction==='down'?'#ff608b':'#ffce7b';document.documentElement.style.setProperty('--signal',signal);
 $('clock').textContent=`10:${String(30+state.step*2).padStart(2,'0')} ET`; $('spot').textContent=m.spot.toFixed(2);$('flow').textContent=m.flow>0?'Comprador':'Vendedor';
 $('verdict').textContent=m.direction==='up'?'RECUPERACIÓN ALCISTA · PRECIO Y FLUJO COINCIDEN':m.direction==='down'?'CONTINUACIÓN BAJISTA · PRECIO Y FLUJO COINCIDEN':m.flat?'PRESIÓN VENDEDORA · PRECIO SIN DESPLAZAMIENTO':'EN ESPERA · REACCIÓN AÚN SIN CONFIRMAR';
 $('why').textContent=m.direction==='up'?`Dos cierres sobre VWAP ${m.vwap.toFixed(2)}. Próxima referencia ${m.up}; exige sostener la recuperación.`:m.direction==='down'?`Movimiento reciente bajista y cierres bajo VWAP ${m.vwap.toFixed(2)}. Referencia inferior ${m.down}; el contexto de dealers no sustituye al precio.`:m.flat?'La presión vendedora no produce una caída proporcional. Posible absorción: falta recuperación del nivel y mejora del flujo.':'Falta combinación de movimiento, cierres sostenidos y flujo. No se asigna una dirección confirmada.';
 $('confirmation').textContent=m.direction==='wait'?'Pendiente':'Precio + flujo';
 $('upperVoid').textContent=m.voids.upper?`${m.voids.upper.low.toFixed(2)}–${m.voids.upper.high.toFixed(2)}`:'Sin zona identificada';
 $('lowerVoid').textContent=m.voids.lower?`${m.voids.lower.low.toFixed(2)}–${m.voids.lower.high.toFixed(2)}`:'Sin zona identificada';
 $('magnet').textContent=m.magnet;$('magnetNote').textContent='Referencia ilustrativa · no es un destino garantizado';
 const ranked=m.rows.slice().sort((a,b)=>b.score-a.score);
 const gaining=m.rows.filter(r=>r.delta>0).sort((a,b)=>b.delta-a.delta)[0]??ranked[0];
 $('strongStrike').textContent=gaining.k;$('strongScore').textContent=`${gaining.score.toFixed(0)}/100 · ${gaining.delta===null?'Sin comparación':points(gaining.delta)}`;
 const reading=transitionReading({path:m.path,flow:m.flow,vwap:m.vwap});
 $('movementReading').dataset.state=reading.kind;
 $('verdict').textContent=reading.headline;$('why').textContent=reading.detail;
 $('readingMoment').textContent=$('clock').textContent;
 const bias=reading.bias;
 const markers=levelMarkers(m.rows,m.magnet);
 const barriers=routeBarriers(m.rows,m.magnet,730,745);
 const pulls=directionalPulls(m.rows,m.spot,730,745);
 for(const side of ['call','put']){const pull=pulls[side];$(`${side}PullStrike`).textContent=pull?pull.target.toFixed(2):'—';$(`${side}PullStrength`).textContent=pull?`${Math.round(pull.strength)}%`:'—';$(`${side}PullFill`).style.width=`${pull?.strength??0}%`;}
 for(const side of ['upper','lower'])$(`${side}RouteBarriers`).innerHTML=barriers[side].map(b=>`<span class="route-barrier-level"><b>${b.strike.toFixed(2)}</b><small>${Math.round(b.strength)}%</small></span>`).join('')||'<span class="route-barrier-empty">Sin barreras intermedias identificadas</span>';
 const percent=v=>v===null?'—':`${Math.round(v)}%`;
 const markerHtml=marker=>`<article class="level-marker marker-${marker.id}${marker.dominant?' is-dominant':''}"><header><span>${marker.label}</span>${marker.dominant?'<em class="dominant-badge">Dominante</em>':''}<b>${marker.strike??'—'}</b></header><div class="marker-importance"><span>Importancia</span><strong>${percent(marker.importance)}</strong></div><div class="marker-track importance-track"><i style="width:${marker.importance??0}%"></i></div>${[['Fuerza',marker.dominanceScore],['Liquidez estimada',marker.concentration]].map(([label,value])=>`<div class="marker-fill"><span>${label}</span><div class="marker-track"><i style="width:${value??0}%"></i></div><b>${percent(value)}</b></div>`).join('')}</article>`;
 $('putLevelMarkers').innerHTML=markers.slice(0,2).map(markerHtml).join('');
 $('callLevelMarkers').innerHTML=markers.slice(2).map(markerHtml).join('');
 $('directionDot').style.left=`${50+bias}%`;$('directionFill').style.left=`${Math.min(50,50+bias)}%`;$('directionFill').style.width=`${Math.abs(bias)}%`;$('directionFill').style.background=bias<0?'var(--put)':'var(--positive)';
 $('candidates').innerHTML=ranked.slice(0,3).map(r=>`<div class="candidate"><b>${r.k}</b><div class="track"><i style="width:${r.score}%"></i></div><span>${r.score.toFixed(0)}/100</span></div>`).join('');
 $('changes').innerHTML=[...m.rows.filter(r=>r.delta>0).sort((a,b)=>b.delta-a.delta).slice(0,2),...m.rows.filter(r=>r.delta<0).sort((a,b)=>a.delta-b.delta).slice(0,1)].map(r=>`<div class="change"><span><b>Strike ${r.k}</b><small>${r.score.toFixed(0)}/100 de importancia</small></span><b class="${r.delta>0?'positive':r.delta<0?'put':'muted'}">${r.delta===null?'Sin comparación':`${r.delta>0?'↑':r.delta<0?'↓':'→'} ${points(r.delta)}`}</b></div>`).join('');
 $('routes').innerHTML=`<div class="route up"><b>Si recupera y sostiene ${m.up}</b><br>Referencia siguiente: ${m.up===740?745:750}<br><span class="muted">Confirmar cierres y flujo comprador.</span></div><div class="route down"><b>Si pierde y sostiene bajo ${m.down}</b><br>Referencia siguiente: ${m.down===738.6?735:730}<br><span class="muted">Confirmar cierres y flujo vendedor.</span></div>`;
 $('absorption').textContent=m.flat?'POSIBLE ABSORCIÓN DE VENTAS':'SIN ABSORCIÓN CONFIRMADA';$('absorptionNote').textContent=m.flat?'Ventas con poco desplazamiento. No identifica quién compra ni si abre posición. Exige reacción posterior.':'No deducir absorción de una sola mecha, del OI ni de gamma positiva.';
 const totalCall=m.rows.reduce((a,r)=>a+r.call,0),totalPut=m.rows.reduce((a,r)=>a+r.put,0);$('totalCall').textContent=money(totalCall);$('totalPut').textContent=money(-totalPut);$('totalNet').textContent=money(totalCall-totalPut);$('totalNet').className=totalCall>=totalPut?'positive':'put';
 $('dominants').innerHTML=ranked.slice(0,5).map(r=>`<div class="dominant"><b>${r.k}</b><div class="track"><i style="width:${r.score}%"></i></div><span>${r.score.toFixed(0)}/100</span></div>`).join('');
 const maxG=Math.max(...m.rows.map(r=>Math.max(r.call,r.put)));
 $('ladder').innerHTML=m.rows.slice().reverse().filter(r=>!$('onlyImportant').checked||r.score>=50).map(r=>`<tr tabindex="0" data-strike="${r.k}" class="${r.k===state.selected?'selected':''}" aria-label="Seleccionar strike ${r.k}"><td><b>${r.k}</b></td><td class="call"><span class="mini track"><i style="width:${r.call/maxG*100}%"></i></span>${money(r.call)}</td><td class="put"><span class="mini track"><i style="width:${r.put/maxG*100}%;background:var(--put)"></i></span>${money(r.put)}</td><td class="${r.net>=0?'positive':'put'}">${money(r.net)}</td><td>${r.score.toFixed(0)}/100</td><td class="${r.delta>0?'positive':'put'}">${r.delta===null?'—':points(r.delta)}</td><td>${money(r.premium)}</td><td>${r.oiCall.toLocaleString('es')} / ${r.oiPut.toLocaleString('es')}</td></tr>`).join('');
 const selected=m.rows.find(r=>r.k===state.selected);$('selected').innerHTML=`<h3>Strike ${selected.k} · detalle</h3><p>Gamma neta: <b class="${selected.net>=0?'positive':'put'}">${money(selected.net)} por 1%</b></p><p>Importancia: ${selected.score.toFixed(1)}/100 · cambio ${selected.delta===null?'sin comparación':points(selected.delta)}</p><p>Prima 5m: ${money(selected.premium)} · volumen 5m: ${selected.volume.toLocaleString('es')} contratos.</p><p>OI del cierre anterior: ${selected.oiCall.toLocaleString('es')} CALL / ${selected.oiPut.toLocaleString('es')} PUT. No demuestra aperturas nuevas.</p>`;
 draw(m);$('scenario').value=state.scenario;
}
let tv,priceSeries,volumeSeries,vwapSeries,priceLines=[],latestModel,previousKey='',profileSignature='',voidSignature='',dominantSignature='';
function syncDominantLabels(height){
 const visible=(latestModel.dominantStrikes??[]).map(strike=>({strike,y:priceSeries.priceToCoordinate(strike)})).filter(r=>r.y!==null&&r.y>50&&r.y<height-35);
 const signature=visible.map(r=>`${r.strike}:${r.y.toFixed(2)}`).join('|');
 if(signature===dominantSignature)return;dominantSignature=signature;
 $('dominantLabels').innerHTML=visible.map(r=>`<div class="dominant-strike-tick" data-dominant-strike="${r.strike}" style="top:${r.y}px"></div>`).join('');
}
function syncVoidLabels(height){
 const zones=[['upper','Vacío superior',latestModel.voids.upper],['lower','Vacío inferior',latestModel.voids.lower]];
 const visible=zones.map(([side,name,zone])=>({side,name,zone,y:zone?priceSeries.priceToCoordinate((zone.low+zone.high)/2):null})).filter(z=>z.y!==null&&z.y>20&&z.y<height-45);
 const signature=visible.map(z=>`${z.side}:${z.zone.low}:${z.zone.high}:${z.y.toFixed(2)}`).join('|');
 if(signature===voidSignature)return;voidSignature=signature;
 $('voidLabels').innerHTML=visible.map(z=>`<div class="void-map-label ${z.side}" data-void="${z.side}" style="top:${z.y}px"><b>${z.name}</b><span>${z.zone.low.toFixed(2)}–${z.zone.high.toFixed(2)}</span></div>`).join('');
}
function initChart(){
 $('chart').innerHTML='<div id="tv-price"></div><aside id="tv-gamma"><header>Gamma y OI por strike<small class="profile-columns"><span>Strike</span><span>OI PUT</span><span>Gamma − / +</span><span>OI CALL</span></small></header><svg id="gammaProfile" preserveAspectRatio="none" aria-label="Strikes, gamma e interés abierto PUT y CALL alineados con los precios"></svg><span class="profile-oi-note">OI · cierre anterior · simulado</span></aside>';
 tv=createChart($('tv-price'),{autoSize:true,layout:{background:{type:ColorType.Solid,color:'#061326'},textColor:'#9aabc0',fontFamily:'Inter Variable',fontSize:13,attributionLogo:true},grid:{vertLines:{visible:false},horzLines:{visible:false}},crosshair:{mode:CrosshairMode.Normal},rightPriceScale:{borderColor:'#244a70',scaleMargins:{top:.08,bottom:.20}},timeScale:{borderColor:'#244a70',timeVisible:true,secondsVisible:false,tickMarkFormatter:t=>state.minutes===1440?new Date(t*1000).toLocaleDateString('es',{timeZone:'UTC',day:'2-digit',month:'short'}):new Date(t*1000).toLocaleTimeString('es',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit'}),rightOffset:6,barSpacing:9,fixLeftEdge:false,fixRightEdge:false},handleScroll:{mouseWheel:true,pressedMouseMove:true,horzTouchDrag:true,vertTouchDrag:true},handleScale:{mouseWheel:true,pinch:true,axisPressedMouseMove:{time:true,price:true},axisDoubleClickReset:{time:true,price:true}},localization:{locale:'es',timeFormatter:t=>new Date(t*1000).toLocaleTimeString('es',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit'})}});
 priceSeries=tv.addSeries(CandlestickSeries,{upColor:'#17d6a2',downColor:'#ff6273',borderVisible:false,wickUpColor:'#17d6a2',wickDownColor:'#ff6273',priceLineColor:'#35d4df',lastValueVisible:true});
 volumeSeries=tv.addSeries(HistogramSeries,{priceFormat:{type:'volume'},priceScaleId:'volume',lastValueVisible:false,priceLineVisible:false});
 vwapSeries=tv.addSeries(LineSeries,{color:'#ff9800',lineWidth:2,lastValueVisible:false,priceLineVisible:false,crosshairMarkerVisible:false});
 $('tv-price').insertAdjacentHTML('beforeend','<div id="voidLabels" aria-label="Vacíos de gamma visibles"></div><div id="dominantLabels" aria-label="Rayitas moradas de strikes dominantes"></div>');
 volumeSeries.priceScale().applyOptions({scaleMargins:{top:.86,bottom:0}});
 tv.subscribeCrosshairMove(e=>{if(!e.point){$('tooltip').hidden=true;return;}const k=Math.round(priceSeries.coordinateToPrice(e.point.y)),r=latestModel?.rows.find(r=>r.k===k);if(!r){$('tooltip').hidden=true;return;}$('tooltip').innerHTML=`Strike ${k} · CALL ${money(r.call)} · PUT ${money(-r.put)}<br>Importancia ${r.score.toFixed(0)}/100`;$('tooltip').hidden=false;$('tooltip').style.left=Math.max(0,Math.min(e.point.x+15,$('chart').clientWidth-290))+'px';$('tooltip').style.top=(e.point.y+55)+'px';});
 tv.subscribeClick(e=>{if(e.point){const k=Math.round(priceSeries.coordinateToPrice(e.point.y));if(strikes.includes(k)){state.selected=k;render();}}});
 const sync=()=>{if(latestModel){syncVoidLabels(document.getElementById("tv-price").clientHeight);syncDominantLabels(document.getElementById("tv-price").clientHeight);const height=$('tv-price').clientHeight,rows=latestModel.rows.map(r=>({...r,y:priceSeries.priceToCoordinate(r.k)})).filter(r=>r.y!==null&&r.y>=50&&r.y<height-35),sig=rows.map(r=>`${r.k}:${r.y.toFixed(2)}:${r.call.toFixed(2)}:${r.put.toFixed(2)}:${r.oiCall}:${r.oiPut}:${state.selected}:${latestModel.dominantStrikes.join(",")}`).join('|');if(sig!==profileSignature){profileSignature=sig;const max=Math.max(...latestModel.rows.map(r=>Math.max(r.call,r.put)));const spacing=rows.length>1?Math.abs(rows[1].y-rows[0].y):20,every=Math.max(1,Math.ceil(18/Math.max(spacing,1)));$('gammaProfile').setAttribute('viewBox',`0 0 390 ${height}`);$('gammaProfile').innerHTML=`<line x1="212" x2="212" y1="50" y2="${height-35}" stroke="#40516b"/>`+rows.map((r,i)=>{const labelled=i%every===0,dominant=latestModel.dominantStrikes.includes(r.k);return `<g data-profile-strike="${r.k}" data-price-y="${r.y}">${dominant?`<line data-profile-dominant="${r.k}" x1="135" x2="290" y1="${r.y}" y2="${r.y}" stroke="#b68bff" stroke-width="2" stroke-dasharray="5 4"/>`:''}<title>Strike ${r.k} · OI PUT ${r.oiPut.toLocaleString('es')} · OI CALL ${r.oiCall.toLocaleString('es')} · cierre anterior simulado · Gamma PUT ${money(-r.put)} / CALL ${money(r.call)}</title>${labelled?`<text class="profile-strike" x="12" y="${r.y+4}" fill="${r.k===state.selected?'#ffffff':'#b9cbe2'}" font-size="14" font-weight="600">${r.k}</text><text class="profile-oi-put" x="124" y="${r.y+4}" text-anchor="end" fill="#ff8795" font-size="13">${r.oiPut.toLocaleString('es')}</text><text class="profile-oi-call" x="306" y="${r.y+4}" fill="#64dbe3" font-size="13">${r.oiCall.toLocaleString('es')}</text>`:''}<rect x="${212-r.put/max*76}" y="${r.y-3}" width="${r.put/max*76}" height="6" fill="#ff6273"/><rect x="212" y="${r.y-3}" width="${r.call/max*76}" height="6" fill="#35d4df"/></g>`;}).join('');}}requestAnimationFrame(sync);};requestAnimationFrame(sync);
}
function draw(m){
 m.dominantStrikes=[...new Set(levelMarkers(m.rows,m.magnet).filter(marker=>marker.dominant).map(marker=>marker.strike))];
 latestModel=m;if(!tv)initChart();
 const source=demoCandles(m.path);
 const bars=aggregate(source,state.minutes);
 priceSeries.setData(bars.map(b=>({time:b.timestampMs/1000,open:b.open,high:b.high,low:b.low,close:b.close})));
 volumeSeries.setData(bars.map(b=>({time:b.timestampMs/1000,value:b.volume,color:b.close>=b.open?'#17d6a250':'#ff627350'})));
 const vwapBuckets=new Map();for(const b of source)vwapBuckets.set(Math.floor(b.timestampMs/(state.minutes*60000))*state.minutes*60000,b.vwap);
 vwapSeries.setData(bars.map(b=>({time:b.timestampMs/1000,value:vwapBuckets.get(b.timestampMs)})));
 priceLines.forEach(l=>priceSeries.removePriceLine(l));priceLines=[];
 for(const [price,title,color] of [[745,'Call Wall','#35d4df'],[730,'Put Wall','#ff6273'],[m.magnet,'Imán','#e8c66e']])priceLines.push(priceSeries.createPriceLine({price,title,color,lineWidth:1,lineStyle:2,axisLabelVisible:true}));
 for(const [zone,name,color] of [[m.voids.upper,'Vacío superior','#75a9bd'],[m.voids.lower,'Vacío inferior','#b98799']])if(zone){
  priceLines.push(priceSeries.createPriceLine({price:zone.high,title:`${name} ${zone.low.toFixed(2)}–${zone.high.toFixed(2)}`,color,lineWidth:1,lineStyle:3,axisLabelVisible:false}));
  priceLines.push(priceSeries.createPriceLine({price:zone.low,title:'',color,lineWidth:1,lineStyle:3,axisLabelVisible:false}));
 }
 const key=state.scenario+':'+state.minutes;if(key!==previousKey){tv.priceScale('right').setAutoScale(true);tv.timeScale().fitContent();previousKey=key;}
 $('rangeLabel').textContent='Rueda: zoom · arrastrar: mover · ejes: ajustar escala';
}
function zoomTime(factor){const r=tv.timeScale().getVisibleLogicalRange();if(r){const mid=(r.from+r.to)/2,half=(r.to-r.from)*factor/2;tv.timeScale().setVisibleLogicalRange({from:mid-half,to:mid+half});}}

$('scenario').onchange=e=>{state.scenario=e.target.value;state.center=null;render();};
document.querySelectorAll('[data-minutes]').forEach(b=>b.onclick=()=>{state.minutes=Number(b.dataset.minutes);document.querySelectorAll('[data-minutes]').forEach(t=>t.setAttribute('aria-pressed',String(t===b)));render();});
$('time').oninput=e=>{state.step=Number(e.target.value);render();};$('onlyImportant').onchange=render;
$('ladder').onclick=e=>{const row=e.target.closest('[data-strike]');if(row){state.selected=Number(row.dataset.strike);render();}};
$('ladder').onkeydown=e=>{if(e.key==='Enter'||e.key===' '){const row=e.target.closest('[data-strike]');if(row){e.preventDefault();state.selected=Number(row.dataset.strike);render();}}};
$('zoomIn').onclick=()=>zoomTime(.8);$('zoomOut').onclick=()=>zoomTime(1.25);$('follow').onclick=()=>{tv.priceScale('right').setAutoScale(true);tv.timeScale().scrollToRealTime();};$('reset').onclick=()=>{state.step=8;$('time').value=8;previousKey='';render();};
$('play').onclick=()=>{state.playing=!state.playing;if(state.playing&&state.step===12)state.step=0;$('play').textContent=state.playing?'Pausar':'Reproducir';render();};
setInterval(()=>{if(!state.playing)return;state.step++;if(state.step>=12){state.step=12;state.playing=false;$('play').textContent='Reproducir';}$('time').value=state.step;render();},1100);
render();
