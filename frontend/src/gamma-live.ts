import {auctionChartBarsSvg,chartAuctionProfile} from './auction-chart-bars';
import {auctionDisplayMemory} from './auction-display-memory';
import './auction-chart-bars.css';
import {readExecutionAbsorption,absorptionCandidateDetail} from './execution-absorption-reading';
import {absorptionOverlaySvg} from './execution-absorption-overlay';
import {inventoryProfile,requestInventory} from './contract-inventory';
import {reportOiProfile} from './option-oi-profile';
import {chartBigTrades} from './big-trades';
import {bigTradeCards,BigTradeTape,bigTradeToolbar} from './big-trade-cards';
import {intradayRoutePanel} from './session-intraday-route';
import {cvdChartEngine} from './cvd-chart-engine';
import {rangeFresh as profileSnapshotFresh} from './daily-range';
import {routeProfileSvg} from './route-gamma-profiles';
import './route-gamma-profiles.css';
import {requestFourHour,currentFourHour,stopFourHour} from './route-4h-live';
import {strikeBidAsk,bidAskSvg,type StrikeFlow} from './strike-bid-ask';
import {retainedContractProfile} from './last-contract-profile';
import {profileExpirations,historicalProfileReference,type ProfileHorizon} from './profile-expirations';
import {chartContractReport} from './contract-profile';
import {strikeHeatmap} from './strike-heatmap';
import {chartMarketSignals} from './market-signals';
import {ChartAlarmPicker,ChartAlerts,chartAlertsToolbar} from './chart-alerts';
import {scrollStrikeCenter,strikeProfileWindow} from './strike-profile-scroll';
import {RouteDrawingTools,routeDrawingToolbar} from './route-drawing-tools';
import {routeChartScenario,routeChartStatus} from './route-chart-scenario';
import {readPriceNow} from './price-now';
import {chartTechnical} from './chart-technical';
import {assessRoute} from './route-assessment';
import {routeChartReading} from './route-live';
import {routeLevelDestinations} from './route-level-destinations';
import {highestArrivalProbability} from './arrival-probability-selection';
import {routePercentLabel} from './route-probability-reading';
import {interpretationFresh} from './confirmations';
import './route-chart.css';
import {structuralMapPreview} from './structural-map-preview';
import {optionActivityPanel} from './option-activity-panel';
import {gammaChartData} from './gamma-chart-data';
import {extendedChartHistory} from './extended-chart-history';
import {gammaReading} from './gamma-reading';
import {createChart,createSeriesMarkers,CandlestickSeries,LineSeries,ColorType,CrosshairMode,type AutoscaleInfo,type IChartApi,type ISeriesApi} from 'lightweight-charts';
import type {LiveState} from './live';
import {escapeHtml as esc,price} from './model';
import {gammaLiveModel,gammaPriceRange,gammaContractRows,type GammaLevel} from './gamma-live-model';
import {buildIntradayControl,ControlTracker,controlPanel} from './intraday-control';
import {controlOutlook} from './control-outlook';
import './gamma-live.css';
let interval=2,expanded=false,profileMetric:"gex"|"oi"|"volume"|"bidask"="oi",profileCenter:number|undefined,profileSymbol='';
let profileHorizon:ProfileHorizon='NEXT4';
let capturedProfileClock:number|undefined;
/** Saved-data viewer uses its capture clock; desktop defaults remain wall-clock based. */
export function setCapturedProfileClock(at:number|undefined){capturedProfileClock=at;}
const profileNow=()=>capturedProfileClock??Date.now();
function profileHorizonTools(s:LiveState|undefined,symbol:string,now:number){
 const report=s?.symbol===symbol?s.optionSnapshotStatus?.structuralEnrichment:undefined;
 const dates=profileExpirations(report,profileHorizon,now);
 if(inventoryProfile(symbol,now))return '<small>Filtros compartidos con Cadena de opciones</small><small class="gl-profile-dates"></small>';
 if(profileMetric==='oi')return '<button data-gl-inventory-refresh>Actualizar OI verificado</button><small class="gl-profile-dates">Filtros compartidos con Cadena de opciones</small>';
 return `<div class="gl-profile-horizons">${([['DAY','0DTE'],['WEEK','Esta semana'],['NEXT4','Próximos 4 vencimientos']] as const).map(([mode,label])=>`<button data-gl-horizon="${mode}" aria-pressed="${profileHorizon===mode}">${label}</button>`).join('')}</div><small class="gl-profile-dates">${esc(dates.length?dates.join(' / '):'Sin vencimientos disponibles en este horizonte')}</small>`;
}


let routeDrawings:RouteDrawingTools|undefined;
let showBigTrades=true,bigTradeMinimum=100000;
const tradeTape=new BigTradeTape();
let selectedBigTrade:number|undefined;
let showExecutionAbsorption=(()=>{try{return localStorage.getItem('snipers-show-execution-absorption')!=='false';}catch{return true;}})(),executionAbsorptionRead:import('./cvd').CvdRead|undefined,executionAbsorptionSymbol='',executionAbsorptionActive=false;
let executionAbsorptionBars:number[]=[];
function paintExecutionAbsorption(){
 if(!host||!chart||!candles)return;
 let svg=host.querySelector<SVGSVGElement>('.gl-execution-absorption');
 if(!svg){svg=document.createElementNS('http://www.w3.org/2000/svg','svg');svg.classList.add('gl-execution-absorption');svg.setAttribute('aria-label','Zonas de posible absorción del activo');host.appendChild(svg);}
 const width=host.clientWidth,height=chart.panes()[0].getHeight();svg.setAttribute('width',String(width));svg.setAttribute('height',String(height));
 const view=readExecutionAbsorption(executionAbsorptionRead,executionAbsorptionSymbol,executionAbsorptionActive,Date.now());
 const html=showExecutionAbsorption?absorptionOverlaySvg(view,c=>{
  let index=executionAbsorptionBars.length-1;while(index>=0&&executionAbsorptionBars[index]>c.lastMs)index--;
  const at=executionAbsorptionBars[index];if(at==null||c.lastMs-at>=interval*60000)return null;
  const x=chart!.timeScale().timeToCoordinate(at/1000 as import('lightweight-charts').UTCTimestamp),y=candles!.priceToCoordinate(c.price);
  return x==null||y==null?null:{x,y};
 },width,height):'';if(svg.innerHTML!==html)svg.innerHTML=html;
}
let chartAuctionReading:import('./auction-map-panel').AuctionReading|undefined,chartAuctionSymbol='',chartAuctionActive=false;
function paintAuctionBars(){
 if(!host||!chart||!candles)return;
 let svg=host.querySelector<SVGSVGElement>('.gl-amt-bars');
 if(!svg){svg=document.createElementNS('http://www.w3.org/2000/svg','svg');svg.classList.add('gl-amt-bars');svg.setAttribute('aria-label','Barras AMT dentro del gráfico, borde izquierdo');svg.setAttribute('role','img');host.appendChild(svg);}
 const now=Date.now(),view=chartAuctionProfile(chartAuctionSymbol,chartAuctionActive,now,chartAuctionReading,auctionDisplayMemory.read(chartAuctionSymbol,now));
 if(view?.state==='live')auctionDisplayMemory.remember(view.reading);
 const width=host.clientWidth,height=chart.panes()[0].getHeight();svg.setAttribute('width',String(width));svg.setAttribute('height',String(height));svg.setAttribute('data-state',view?.state??'pending');
 const html=auctionChartBarsSvg(view,p=>candles!.priceToCoordinate(p),width,height,now);if(svg.innerHTML!==html)svg.innerHTML=html;
}
function paintBigTrades(){host?.querySelector('.gl-big-trades')?.remove();}
let chartAlerts:ChartAlerts|undefined;let chartAlarmPicker:ChartAlarmPicker|undefined;
let chart:IChartApi|undefined,candles:ISeriesApi<'Candlestick'>|undefined,vol:ISeriesApi<'Line'>|undefined,vwap:ISeriesApi<'Line'>|undefined,host:HTMLElement|undefined;
let lines:ReturnType<ISeriesApi<'Candlestick'>['createPriceLine']>[]=[],chartKey='',profileKey='',dataKey='',lineKey='',latest:ReturnType<typeof gammaLiveModel>|undefined,scaleFrame=0;
const pct=(n:number|null|undefined)=>n!=null&&Number.isFinite(n)?`${Math.round(n)}%`:'—';
const metric=(name:string,n:number|null|undefined)=>`<div class="gl-metric"><span>${name}</span><div><i style="width:${Math.max(0,Math.min(100,n??0))}%"></i></div><b>${pct(n)}</b></div>`;
const card=(l:GammaLevel)=>`<article class="gl-level gl-${l.id}"><header><span>${l.label}</span>${l.dominant?'<em>Dominante</em>':''}<b>${l.strike==null?'—':price(l.strike)}</b></header><div class="gl-importance"><span>Importancia</span><b>${pct(l.importance)}</b></div>${metric('Fuerza gamma/GEX',l.force)}${metric('Liquidez estimada',l.liquidity)}</article>`;
export function gammaLivePage(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now()):string {
 const {model:m,reading,movement}=gammaReading(s,symbol,active,now),outlook=controlOutlook(reading);

 const quote=reading.priceNow?.spot,dir=movement.direction,bias=movement.bias,headline=movement.headline;
 const control=controlPanel(reading,true,structuralMapPreview(symbol,s,active,now)+optionActivityPanel(symbol,active,now));
 const route=(upper:boolean)=>{const r=upper?m.call:m.put,barriers=upper?m.upper:m.lower;return `<div class="gl-route ${upper?'gl-upper':'gl-lower'}"><div><span>Imán principal → ${upper?'Call':'Put'} Wall</span><div class="gl-barriers">${barriers.map(b=>`<span><b>${price(b.strike)}</b> ${pct(b.score)}</span>`).join('')||'<small>Sin barreras intermedias verificadas</small>'}</div></div><div class="gl-pull"><span>${upper?'CALL':'PUT'} PULL</span><strong>${r?.pullTarget==null?'—':price(r.pullTarget)}</strong><b>${pct(r?.score)}</b>${metric('Fuerza',r?.score)}</div></div>`;};
 const zone=(r:typeof m.call)=>r?.vacuumLow!=null&&r.vacuumHigh!=null?`${price(r.vacuumLow)}–${price(r.vacuumHigh)}`:'—';
 return `<section class="gamma-live ${expanded?'gl-expanded':''}" aria-label="Mapa gamma en vivo"><header class="gl-header"><div><strong>${esc(symbol)} · MAPA GAMMA EN VIVO</strong><b>${quote==null?'—':price(quote)} <small>USD</small></b></div><span>${esc(m.status)} · <small id="chart-history-days">0/30 sesiones</small></span></header><div class="gl-tools"><div>${[1,2,5,15,60,240,1440].map(n=>`<button data-gl-interval="${n}" aria-pressed="${interval===n}">${n===60?'1h':n===240?'4h':n===1440?'1D':n+'m'}</button>`).join('')}</div><small>Rueda: zoom · arrastra: mover · ejes: ajustar escala</small><button data-gl-action="follow">Seguir precio</button><button data-gl-action="reset">Restablecer</button><button data-refresh-options ${!active||!s?.selection?.spot||s?.optionSnapshotStatus?.phase==='loading'?'disabled':''}>Actualizar gamma/OI</button><button data-gl-action="expand">${expanded?'Reducir':'Ampliar'}</button>${chartAlertsToolbar(symbol)}${bigTradeToolbar()}</div><div class="gl-chart"><div id="gamma-live-chart" data-chart-preserve></div><aside class="gl-profile"><header>Perfil por strike · ${profileMetric==="gex"?"GEX":profileMetric==="oi"?"OI":profileMetric==="bidask"?"BID/ASK":"VOLUMEN"}<small>PUT ← · → CALL · ${profileMetric==="gex"?"USD por movimiento del 1 %":profileMetric==="oi"?"contratos abiertos":profileMetric==="bidask"?"ASK verde · BID rojo · muestra 5 min":"volumen acumulado · snapshot"}</small><div><button data-gl-metric="gex" aria-pressed="${profileMetric==='gex'}">GEX</button><button data-gl-metric="oi" aria-pressed="${profileMetric==='oi'}">Contratos abiertos (OI)</button><button data-gl-metric="volume" aria-pressed="${profileMetric==='volume'}">Volumen</button><button data-gl-metric="bidask" aria-pressed="${profileMetric==='bidask'}">BID/ASK</button><button data-gl-profile-center>Centrar</button></div>${profileHorizonTools(s,symbol,now)}</header><svg id="gamma-live-profile" data-chart-preserve aria-label="Gamma y contratos abiertos por strike"></svg><small>${m.complete?"Perfil completo":"Perfil parcial · datos observados"}${profileMetric==="gex"&&!m.h?.structure.gexAvailable?` · ${m.h?.structure.provisional?.coveredContracts??0}/${m.h?.structure.provisional?.totalContracts??0} contratos`:""} · Snapshot</small></aside><div class="gl-empty" ${active&&s?.chartHistory?.minutes.length?'hidden':''}>Esperando velas reales de ${esc(symbol)}.</div></div><section class="gl-reading"><header><strong>LECTURA DEL MOVIMIENTO</strong><small>PRECIO Y FLUJO · SNIPERS</small></header><h3>${esc(headline)}</h3><p>${esc(movement.detail)}</p><div class="gl-levels">${m.levels.map(card).join('')}${route(true)}<div class="gl-direction"><b>PUT</b><div><i style="left:${Math.min(50,50+bias)}%;width:${Math.abs(bias)}%;background:${dir<0?'#ff535f':'#49ff68'}"></i><span></span><em style="left:${50+bias}%"></em></div><b>CALL</b></div>${route(false)}</div><details data-horizon-details="gamma-live-method"><summary>Condiciones de esta lectura</summary><p>${esc(outlook.changeDetail)} ${esc(outlook.routeDetail)}</p><p>Muros, M1, imán de liquidez, pull y vacíos de la misma lectura Snipers. El perfil parcial muestra únicamente datos observados; sus totales pueden estar incompletos. La escala automática incluye los muros y vacíos actuales. Universo DAY; si falta, referencias del próximo vencimiento. Importancia: score estructural de Snipers. Fuerza: 50% GEX bruto, 20% magnitud del GEX neto y 20% gamma media ponderada por OI, normalizados entre strikes; sin historial de GEX comparable, esos pesos se renormalizan a 100%. Dominante: hasta dos strikes distintos a menos de dos puntos del líder; si dos tarjetas representan el mismo strike, comparten la marca. Liquidez estimada: concentración relativa de OI, volumen del snapshot, GEX e importancia; no mide órdenes en espera. Barreras intermedias: strikes con score de Snipers ≥50. Puntuaciones del modelo, no probabilidades. Las referencias pueden mostrarse provisionales, sin puntuaciones; la fuerza y la dominancia requieren cadena completa y GEX vigente.</p><p>Un cambio CALL/PUT exige un sesgo anterior observado y dos observaciones diferentes separadas por al menos 2 s con precio y flujo completos alineados. Perder velocidad no confirma un giro. La desconexión, el cambio de activo o sesión y la evidencia caducada reinician la lectura. Precio y velas se actualizan con la conexión compartida. Gamma y OI cambian al recibir un nuevo snapshot; no tick a tick. Las velas históricas no se presentan con exposición actual. VWAP naranja solo donde existe cobertura válida.</p></details></section><div class="gl-context"><span>Vacío superior <b>${zone(m.call)}</b></span><span>Gamma Flip <b>${m.flip==null?'—':price(m.flip)}</b></span><span>Vacío inferior <b>${zone(m.put)}</b></span></div><div class="gl-control">${control}</div><p class="gl-note">Snapshot ${m.fresh&&m.h?esc(new Date(m.h.observedMs).toLocaleString('es',{timeZone:'America/New_York'}))+' ET':'pendiente'} · ${esc(m.h?.expirations.join(' / ')??'Sin vencimientos')} · lectura visual; sin cambios en las entradas del bot.</p><a class="gl-attribution" href="https://www.tradingview.com/" target="_blank" rel="noopener noreferrer">TradingView Lightweight Charts</a></section>`;
}
/** Same renderer/provider/profile as gamma view, with price-route overlays only here. */
let dualVisible=false,showSwing=false;
export function gammaRouteChart(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now(),dualEnabled=false):string {
 const m=gammaLiveModel(s,symbol,active,now);
 return `<section class="gamma-live gl-route-chart ${dualEnabled&&dualVisible?'gl-dual-visible':''} ${expanded?'gl-expanded':''}" aria-label="Gráfica de ruta del precio"><header class="gl-header"><div><strong>${esc(symbol)} · PRECIO, ESTRUCTURA Y RECORRIDO</strong>${chartAlertsToolbar(symbol)}</div><span>${esc(m.status)} · <small id="chart-history-days">0/30 sesiones</small></span></header><div class="gl-tools"><div>${[1,2,5,15,60,240,1440].map(n=>`<button data-gl-interval="${n}" aria-pressed="${interval===n}">${n===60?'1h':n===240?'4h':n===1440?'1D':n+'m'}</button>`).join('')}</div>${dualEnabled?`<button data-route-profiles aria-pressed="${dualVisible}">${dualVisible?'Ocultar gamma / GEX':'Mostrar gamma / GEX'}</button>`:''}<button data-route-swing aria-pressed="${showSwing}">${showSwing?'Ocultar contexto 4H':'Mostrar contexto 4H'}</button><button data-route-overlay="structure" aria-pressed="${showStructure}">Estructura</button><button data-gl-action="follow">Seguir precio</button><button data-gl-action="reset">Restablecer</button><button data-gl-action="expand">${expanded?'Reducir':'Ampliar'}</button>${routeDrawingToolbar(symbol)}${bigTradeToolbar()}</div><div class="gl-chart">${dualEnabled?`<aside class="gl-dual-profile"><header>GAMMA · + / −</header><svg data-route-gamma data-chart-preserve></svg></aside><aside class="gl-dual-profile"><header>GEX NETO · USD / 1%</header><svg data-route-gex data-chart-preserve></svg></aside>`:''}<div id="gamma-live-chart" data-chart-preserve></div><aside class="gl-profile"><header>Perfil por strike · ${profileMetric==='gex'?'GEX':profileMetric==='oi'?'OI':profileMetric==='bidask'?'BID/ASK':'VOLUMEN'}<small>PUT ← · → CALL · ${profileMetric==='oi'?'contratos abiertos':profileMetric==='volume'?'contratos negociados':profileMetric==='bidask'?'muestra ejecutada · 5 min':'exposición gamma'} · rueda ↑↓</small><div><button data-gl-metric="gex" aria-pressed="${profileMetric==='gex'}">GEX</button><button data-gl-metric="oi" aria-pressed="${profileMetric==='oi'}">Contratos abiertos (OI)</button><button data-gl-metric="volume" aria-pressed="${profileMetric==='volume'}">Volumen</button><button data-gl-metric="bidask" aria-pressed="${profileMetric==='bidask'}">BID/ASK</button><button data-gl-profile-center>Centrar</button></div>${profileHorizonTools(s,symbol,now)}</header><svg id="gamma-live-profile" data-chart-preserve aria-label="Gamma y contratos abiertos por strike"></svg><small>Perfil de opciones · ${esc(m.h?.expirations?.join(' / ')??'vencimiento pendiente')} · no es volumen negociado por precio</small></aside><div class="gl-empty" ${active&&s?.symbol===symbol&&s?.chartHistory?.minutes.length?'hidden':''}>Esperando velas reales de ${esc(symbol)}.</div></div><div id="route-intraday-status" data-chart-preserve>${intradayRoutePanel(undefined)}</div><div id="route-chart-stats" data-chart-preserve class="route-chart-stats">Indicadores: esperando velas cerradas</div><details data-horizon-details="route-chart-method"><summary>Lecturas y condiciones</summary><p id="route-4h-details">Intradía: rango de apertura de 15 min y pivotes de sesión confirmados con dos velas posteriores. Recuperación, prueba y continuación en 1 min; CVD reciente de 5 min con ≥70% de cobertura y dos cierres distintos. Recorrido restante mínimo 1,8 veces la invalidación; opciones comparables contrarias bloquean. Sin 0DTE no se mezclan muros del próximo vencimiento. Confirmación técnica experimental, sin rentabilidad validada ni ejecución automática. Contexto 4H nativo: pivotes confirmados y escenario swing independiente de la ruta intradía. Objetivos de nivel conocido o rango estimado, con invalidación por cierre de minuto. Consulta local cada diez segundos; sin descargas históricas automáticas. La curva no indica hora de llegada ni probabilidad calibrada.</p><p>Velas reales, curva CVD observada, VWAP de sesión y perfil GEX/OI compartidos con Mapa gamma. Temporalidad 4h anclada a 09:30 ET; última barra RTH de 2,5h, cerrada únicamente al finalizar la sesión. Sin bandas de Bollinger. RSI14 y ATR14 de Wilder; MACD12/26/9; MFI14, CMF20 y OBV relativo. Volumen relativo: última vela frente a 20 anteriores de esta temporalidad. Datos incompletos o huecos reinician las ventanas; no se rellenan.</p><p>HH/HL/LH/LL: pivotes estrictos de dos velas a cada lado, publicados solo tras cerrar las dos posteriores; se muestran las últimas 12 marcas técnicas grises con etiquetas breves en la vela de confirmación, dos velas después del pivote; describen estructura y no son señales CALL/PUT. BOS requiere dos cierres sobre/bajo el pivote confirmado; ChoCH? es un cambio candidato, no confirma absorción ni cambia por sí solo el sesgo.</p><p>Las marcas RECUPERACIÓN, PRUEBA, CONTINUACIÓN e INVALIDACIÓN son observaciones de la ruta y comparten la lectura del control, niveles y CALL/PUT. Se publican al conocerse en vivo, sin reconstruir señales pasadas. El color ámbar señala apoyo incompleto, oposición o invalidación; no autoriza órdenes. La flecha es un escenario del motor Ruta del precio hacia su siguiente referencia, solo con continuación sostenida, precio y niveles vigentes. Ámbar y «Solo precio» indican apoyo incompleto; «Presión en contra» señala contradicción. Verde/rojo requieren coincidencia de ruta, sesgo, flujo y CVD recientes. Su longitud horizontal no indica hora de llegada. Invalidación por cierre según la ruta; no se fabrican velas futuras, probabilidades ni ejecuciones. Historial de precio y exposición actual permanecen separados. La curva CVD usa compras agresivas estimadas menos ventas agresivas estimadas mediante operaciones y bid/ask. Solo incluye observaciones desde la conexión; huecos y reconexiones separan los tramos. No se reconstruye con OHLCV. Fórmulas públicas propias; no replica el algoritmo privado de iYurek.</p></details><a class="gl-attribution" href="https://www.tradingview.com/" target="_blank" rel="noopener noreferrer">TradingView Lightweight Charts</a></section>`;
}
let showStructure=true,technicalKey='',technical:ReturnType<typeof chartTechnical>|undefined;
let markers:import('lightweight-charts').ISeriesMarkersPluginApi<import('lightweight-charts').Time>|undefined,routeSvg:SVGSVGElement|undefined,routeSymbol='',routeNow=0,routeCurrent=false,routeSpot:number|undefined,routeChartBarMs:number|undefined,routeStatsPrefix='';
function technicalOverlay(prepared:ReturnType<typeof gammaChartData>,symbol:string,enabled:boolean,s:LiveState|undefined,active:boolean,now:number){
 const spot=readPriceNow(s,symbol,active,now);routeSpot=spot.ready?spot.spot:undefined;
 routeChartBarMs=prepared.bars.at(-1)?.timestampMs;routeSymbol=symbol;routeNow=now;routeCurrent=enabled&&prepared.currentOverlay&&active&&s?.symbol===symbol&&s.stocks.phase==='streaming'&&interpretationFresh(s.publishedMs,now)&&s.metrics.phase==='Sesión RTH'&&spot.ready;
 if(!enabled){routeSvg?.remove();routeSvg=undefined;}
 const key=JSON.stringify([symbol,interval,enabled,showStructure,prepared.closedBars,routeCurrent,routeChartReading(symbol,now)?.signals,routeChartReading(symbol,now)?.intraday.signals,routeChartReading(symbol,now)?.quant.signals]);
 if(key!==technicalKey){technicalKey=key;technical=enabled?chartTechnical(prepared.closedBars,interval,now):undefined;
  if(enabled&&!markers)markers=createSeriesMarkers(candles!,[],{autoScale:false});
  const observed=chartMarketSignals([...(routeChartReading(symbol,now)?.signals??[]),...(routeChartReading(symbol,now)?.intraday.signals??[]),...(routeChartReading(symbol,now)?.quant.signals??[])],prepared.bars,symbol,now,routeCurrent,interval*60000);
  const chartMarks=observed;
  markers?.setMarkers(chartMarks.map(p=>({time:p.timestampMs/1000 as import('lightweight-charts').UTCTimestamp,position:p.above?'aboveBar' as const:'belowBar' as const,color:p.color,shape:p.shape,text:p.text})).sort((a,b)=>Number(a.time)-Number(b.time)));
 }
 const status=document.getElementById('route-chart-stats'),r=routeChartReading(symbol,now),n=(v:number|null|undefined)=>v!=null&&Number.isFinite(v)?v.toFixed(2):'—';
 routeStatsPrefix=`${prepared.historical?'HISTÓRICO · ':''}${interval===1440?'Diario':interval===240?'4h':interval+' min'} · RSI ${n(technical?.rsi)} · MACD ${n(technical?.histogram)} · ATR ${n(technical?.atr)} · Vol ${n(technical?.relativeVolume)}× · MFI ${n(technical?.mfi)} · CMF ${n(technical?.cmf)} · OBV ${n(technical?.obv)}`;
 if(status)status.textContent=`${routeStatsPrefix} · CVD ${routeCurrent?r?.cvd??'Pendiente':'Pendiente'} · ${routeChartStatus(r?.result,routeCurrent,r?.market,now)} · ${r?.intraday.label??'INTRADÍA · esperando prueba'} · ${currentFourHour(symbol,now).summary}`;
 drawRoute();
}
function drawRoute(){
 if(!host||!chart||!candles||!document.querySelector('.gl-route-chart'))return;
 if(!routeSvg){routeSvg=document.createElementNS('http://www.w3.org/2000/svg','svg');routeSvg.classList.add('route-chart-path');host.appendChild(routeSvg);}
 routeSvg.style.clipPath=`inset(0 0 ${Math.max(0,host.clientHeight-chart.panes()[0].getHeight())}px 0)`;
 routeSvg.setAttribute('viewBox',`0 0 ${host.clientWidth} ${host.clientHeight}`);
 const now=Date.now(),last=routeChartBarMs,cached=routeChartReading(routeSymbol,now),current=routeCurrent&&now>=routeNow&&now-routeNow<=3000;
 const signalPanel=document.getElementById('route-intraday-status');if(signalPanel){const html=intradayRoutePanel(current?cached?.intraday:undefined);if(signalPanel.innerHTML!==html)signalPanel.innerHTML=html;}
 const status=document.getElementById('route-chart-stats');if(status){const text=`${routeStatsPrefix} · CVD ${current?cached?.cvd??'Pendiente':'Pendiente'} · ${routeChartStatus(cached?.result,current,cached?.market,now)} · ${cached?.intraday.label??'INTRADÍA · esperando prueba'} · ${currentFourHour(routeSymbol,now).summary}`;if(status.textContent!==text)status.textContent=text;}
 const intraday=cached?.intraday,confirmed=intraday?.state==='confirmed';
 const r=routeChartScenario(confirmed?intraday.result:cached?.result,routeSpot,now,current);
 const assessment=cached?assessRoute(cached.result,cached.market,now):undefined;
 const directional=cached?.directional;
 const highest=current&&cached?highestArrivalProbability(cached.evaluation.probabilities,now):undefined;
 const valid=r&&last!=null&&(!highest||highest.side===r.direction&&highest.target===r.target)&&(confirmed||latest?.fresh&&assessment?.quality!=='pending'&&directional&&['conditional','supported','opposed','reference-test'].includes(directional.state));
 let html='';if(valid){const x=chart.timeScale().timeToCoordinate(last!/1000 as import('lightweight-charts').UTCTimestamp),y=candles.priceToCoordinate(routeSpot!),target=candles.priceToCoordinate(r.target!),stop=candles.priceToCoordinate(r.stop!);const edge=host.clientWidth-85;
  if(x!=null&&y!=null&&target!=null&&x>0&&x<edge-20&&y>0&&y<host.clientHeight-30&&target>20&&target<host.clientHeight-30){
   const end=Math.min(edge,x+110),color=confirmed?(r.direction===1?'#25edac':'#ff6282'):'#ffb564';
   const label=confirmed?'POSIBLE LLEGADA '+intraday!.side+' · precio confirmado':directional?.state==='opposed'?'Ruta · presión en contra':'POSIBLE LLEGADA '+(r.direction===1?'CALL':'PUT')+' · '+(directional?.state==='supported'?'con apoyo':'condicionada');
   const arrival=cached?.evaluation.probabilities.find(p=>p.side===r.direction&&p.target===r.target&&p.horizon==='session');
   const probabilityLabel=arrival?.probability!=null?'P hoy '+(arrival.probability*100).toFixed(1)+'%':'P llegada pendiente';
   html=`<path d="M ${x} ${y} Q ${(x+end)/2} ${y} ${end} ${target}" fill="none" stroke="${color}" stroke-width="2" stroke-dasharray="5 4"/><circle cx="${end}" cy="${target}" r="4" fill="${color}"/><text x="${end-5}" y="${target-9}" text-anchor="end" fill="${color}" font-size="10">${label} $${r.target.toFixed(2)} · ${probabilityLabel}</text>`;
   if(stop!=null&&stop>20&&stop<host.clientHeight-30)html+=`<line x1="${x}" x2="${end}" y1="${stop}" y2="${stop}" stroke="#ffb564" stroke-dasharray="3 4"/><text x="${end-5}" y="${stop+12}" text-anchor="end" fill="#ffb564" font-size="9">Invalida $${r.stop.toFixed(2)}</text>`;
  }
 }
 if(!valid&&current&&cached&&last!=null&&routeSpot!=null){
  const references=routeLevelDestinations(cached.market,now),destinations=highest?references.filter(d=>d.direction===highest.side&&d.target===highest.target):references.length===1?references:[],x=chart.timeScale().timeToCoordinate(last/1000 as import('lightweight-charts').UTCTimestamp),y=candles.priceToCoordinate(routeSpot),edge=host.clientWidth-85;
  if(x!=null&&y!=null&&x>0&&x<edge-20){
   for(const d of destinations){const target=candles.priceToCoordinate(d.target),end=Math.min(edge,x+130),color=d.direction>0?'#25edac':'#ff6282';
    if(target==null||target<20||target>chart.panes()[0].getHeight()-25)continue;
    const probability=cached.evaluation.probabilities.find(p=>p.side===d.direction&&p.target===d.target&&p.horizon==='session'),pct=probability?.probability!=null?'P hoy '+(probability.probability*100).toFixed(1)+'%':'P hoy pendiente';
    html+=`<path d="M ${x} ${y} L ${end} ${target}" fill="none" stroke="${color}" stroke-width="2" stroke-dasharray="3 5"/><circle cx="${end}" cy="${target}" r="3" fill="${color}"/><text x="${end-4}" y="${target+(d.direction>0?-9:14)}" text-anchor="end" fill="${color}" font-size="10">Posible ${d.direction>0?'CALL ↑':'PUT ↓'} $${d.target.toFixed(2)} · ${d.provisional?'provisional · ':''}${pct}</text>`;
   }
  }
  if(status){const labels=destinations.map(d=>`${d.label} · distancia $${d.distance.toFixed(2)} · fuerza ${d.strength==null?'pendiente':d.strength.toFixed(0)+'/100'}`).join(' | ');if(labels)status.textContent+=' · Lectura anticipada: '+labels+' · escenarios alternativos, falta confirmación';}
 }
 if(current&&cached&&status){const probability=highest?`${highest.side>0?'CALL':'PUT'} ${(highest.probability!*100).toFixed(1)}% → $${highest.target.toFixed(2)}`:'pendiente de probabilidades comparables';status.textContent+=' · '+cached.emaContext.label+' · '+cached.evaluation.label+' · Mayor prob. llegada hoy '+probability+' · modelo experimental';}
 const swing=currentFourHour(routeSymbol,now),ss=swing.scenario;
 const details=document.getElementById('route-4h-details');if(details)details.textContent=`${swing.text}. Proyección empírica del mismo activo, señal, estructura y duración de bloque; riesgo relativo entre la mitad y el doble. Zona P25–P75 de excursiones, mediana central, retroceso y minutos RTH hasta el máximo favorable observado antes de invalidarse. Se mide hasta cierre de sesión o dos sesiones RTH, sin cortar al alcanzar el objetivo anterior. Mínimo operativo: 20 casos completos sin solaparse y terminados antes de la señal. El tiempo hasta el pico no es hora de llegada. Los cuartiles no son una cobertura garantizada ni una probabilidad de éxito. Precio histórico solamente; flujo/CVD actuales se usan para confirmación, no se inventa su historial. Referencia estructural separada si falta muestra. Sin descargas automáticas ni nuevos indicadores.`;
 if(showSwing&&ss&&current&&routeSpot!=null&&last!=null&&(ss.target-routeSpot)*ss.side>0&&(routeSpot-ss.invalidation)*ss.side>0){
  const x=chart.timeScale().timeToCoordinate(last/1000 as import('lightweight-charts').UTCTimestamp),y=candles.priceToCoordinate(routeSpot),ty=candles.priceToCoordinate(ss.target),sy=candles.priceToCoordinate(ss.invalidation),edge=host.clientWidth-85;
  if(x!=null&&y!=null&&ty!=null&&x>0&&x<edge-35&&y>20&&y<host.clientHeight-30&&ty>20&&ty<host.clientHeight-30){
   const end=Math.min(edge,x+160),color=swing.supported?'#7db9ff':'#ffb564';
   const projected=swing.projection;
   if(projected){
    const low=candles.priceToCoordinate(projected.low!),high=candles.priceToCoordinate(projected.high!);
    if(low!=null&&high!=null){const top=Math.max(20,Math.min(low,high)),bottom=Math.min(host.clientHeight-30,Math.max(low,high));if(bottom>top)html+=`<rect x="${x+8}" y="${top}" width="${Math.max(0,end-x-8)}" height="${bottom-top}" fill="${color}" fill-opacity="0.10" stroke="${color}" stroke-dasharray="2 5"/>`;}
   }
   html+=`<path d="M ${x} ${y} Q ${(x+end)/2} ${y} ${end} ${ty}" fill="none" stroke="${color}" stroke-width="2" stroke-dasharray="2 5"/><text x="${end-4}" y="${ty+15}" text-anchor="end" fill="${color}" font-size="10">Contexto 4H · ${projected?'mediana histórica':'swing'} $${ss.target.toFixed(2)}${projected?' · n='+projected.n:ss.targetSource==='range-projection'?' · estimado':''}</text>`;
   if(sy!=null&&sy>20&&sy<host.clientHeight-30)html+=`<line x1="${x}" x2="${end}" y1="${sy}" y2="${sy}" stroke="${color}" stroke-dasharray="2 5"/><text x="${end-4}" y="${sy-7}" text-anchor="end" fill="${color}" font-size="9">Invalida 4H $${ss.invalidation.toFixed(2)}</text>`;
  }
 }
 if(routeSvg.innerHTML!==html)routeSvg.innerHTML=html;
}
function cleanup(){chartAuctionReading=undefined;chartAuctionSymbol='';chartAuctionActive=false;executionAbsorptionRead=undefined;executionAbsorptionBars=[];executionAbsorptionActive=false;chartAlarmPicker?.dispose();chartAlarmPicker=undefined;chartAlerts?.dispose();chartAlerts=undefined;routeDrawings?.dispose();routeDrawings=undefined;markers?.detach();markers=undefined;routeSvg?.remove();routeSvg=undefined;technical=undefined;technicalKey='';routeCurrent=false;routeChartBarMs=undefined;routeStatsPrefix='';chart?.remove();chart=undefined;host=undefined;candles=undefined;lines=[];chartKey='';profileKey='';dataKey='';lineKey='';cancelAnimationFrame(scaleFrame);}
export function bindGammaLive(s:LiveState|undefined,symbol:string,active:boolean,refresh:()=>void,now=Date.now()) {
 const root=document.querySelector<HTMLElement>('.gamma-live'),node=document.getElementById('gamma-live-chart');
 if(profileSymbol!==symbol){profileCenter=undefined;profileSymbol=symbol;}
 if(!root||!node){stopFourHour();if(chart)cleanup();return;}
 const inventoryButton=root.querySelector<HTMLButtonElement>('[data-gl-inventory-refresh]');if(inventoryButton)inventoryButton.onclick=()=>{void requestInventory(symbol,refresh,true);};
 if(active&&profileMetric==='oi')queueMicrotask(()=>{if(root.isConnected)void requestInventory(symbol,refresh);});
 const tools=root.querySelector<HTMLElement>('.gl-tools');
 const bigToggle=root.querySelector<HTMLButtonElement>('[data-big-trades]');if(bigToggle){bigToggle.textContent=showBigTrades?'Ocultar trades grandes':'Mostrar trades grandes';bigToggle.setAttribute('aria-pressed',String(showBigTrades));bigToggle.onclick=()=>{showBigTrades=!showBigTrades;refresh();};}
 const absorptionToggle=root.querySelector<HTMLButtonElement>('[data-execution-absorption]');
 const absorptionView=readExecutionAbsorption(s?.cvd,symbol,active,now);
 if(absorptionToggle){absorptionToggle.textContent=showExecutionAbsorption?absorptionView.label:'Mostrar absorción';absorptionToggle.setAttribute('aria-pressed',String(showExecutionAbsorption));absorptionToggle.title=[absorptionView.detail,...absorptionView.candidates.map(absorptionCandidateDetail),'Pulsa para ocultar o mostrar las marcas.'].join(' · ');absorptionToggle.onclick=()=>{showExecutionAbsorption=!showExecutionAbsorption;try{localStorage.setItem('snipers-show-execution-absorption',String(showExecutionAbsorption));}catch{/* Reading works without persisted preferences. */}refresh();};}
 const bigFilter=root.querySelector<HTMLSelectElement>('[data-big-trades-min]');if(bigFilter){bigFilter.value=String(bigTradeMinimum);bigFilter.onchange=()=>{bigTradeMinimum=[100000,250000,1000000].includes(Number(bigFilter.value))?Number(bigFilter.value):100000;refresh();};}

 requestFourHour(symbol,root.classList.contains('gl-route-chart')&&active&&s?.symbol===symbol&&s.stocks.phase==='streaming',now,refresh);
 root.querySelectorAll<HTMLButtonElement>('[data-route-overlay]').forEach(b=>b.onclick=()=>{showStructure=!showStructure;technicalKey='';refresh();});
 const swingToggle=root.querySelector<HTMLButtonElement>('[data-route-swing]');if(swingToggle)swingToggle.onclick=()=>{showSwing=!showSwing;refresh();};
 const toggle=root.querySelector<HTMLButtonElement>('[data-route-profiles]');if(toggle)toggle.onclick=()=>{dualVisible=!dualVisible;refresh();};
 const profileArea=root.querySelector<HTMLElement>('.gl-profile');
 if(profileArea){profileArea.onwheel=e=>{if(e.target instanceof Element&&e.target.closest('.gl-profile-dates'))return;const rows=profileRows();if(!rows.length)return;e.preventDefault();e.stopPropagation();const px=e.deltaY*(e.deltaMode===1?22:e.deltaMode===2?host?.clientHeight??440:1),reference=routeSpot??latest?.levels.find(l=>l.id==='main')?.strike??rows[Math.floor(rows.length/2)].strike;profileCenter=scrollStrikeCenter(rows.map(r=>r.strike),profileCenter,reference,px);profileKey='';profile();};}
 root.querySelectorAll<HTMLButtonElement>('[data-gl-profile-center]').forEach(b=>b.onclick=()=>{profileCenter=undefined;profileKey='';profile();});
 root.querySelectorAll<HTMLButtonElement>('[data-gl-horizon]').forEach(b=>b.onclick=()=>{profileHorizon=b.dataset.glHorizon==='WEEK'?'WEEK':b.dataset.glHorizon==='NEXT4'?'NEXT4':'DAY';profileKey='';profileCenter=undefined;refresh();});
 root.querySelectorAll<HTMLButtonElement>('[data-gl-metric]').forEach(b=>b.onclick=()=>{profileMetric=b.dataset.glMetric==='bidask'?'bidask':b.dataset.glMetric==='volume'?'volume':b.dataset.glMetric==='oi'?'oi':'gex';profileKey='';refresh();});
 root.querySelectorAll<HTMLButtonElement>('[data-gl-interval]').forEach(b=>b.onclick=()=>{interval=Number(b.dataset.glInterval);refresh();});
 root.querySelectorAll<HTMLButtonElement>('[data-gl-action]').forEach(b=>b.onclick=()=>{if(b.dataset.glAction==='expand'){expanded=!expanded;refresh();}else if(b.dataset.glAction==='follow'){chart?.priceScale('right').setAutoScale(true);chart?.timeScale().scrollToRealTime();}else{chart?.priceScale('right').setAutoScale(true);chart?.timeScale().fitContent();}});
 if(host!==node){cleanup();host=node;chart=createChart(node,{autoSize:true,layout:{background:{type:ColorType.Solid,color:'rgba(6,19,38,0)'},textColor:'#9bb7cf',fontFamily:'Inter Variable',fontSize:13,attributionLogo:true,panes:{enableResize:true,separatorColor:"#35d4df",separatorHoverColor:"#17e3be"}},grid:{vertLines:{visible:false},horzLines:{visible:false}},crosshair:{mode:CrosshairMode.Normal},rightPriceScale:{borderColor:'#244a70',scaleMargins:{top:.08,bottom:.08}},timeScale:{borderColor:'#244a70',timeVisible:true,rightOffset:6,barSpacing:9,tickMarkFormatter:(t:import('lightweight-charts').Time)=>interval===1440?new Date(Number(t)*1000).toLocaleDateString('es',{timeZone:'America/New_York',day:'2-digit',month:'short'}):new Date(Number(t)*1000).toLocaleTimeString('es',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit'})},handleScroll:{mouseWheel:true,pressedMouseMove:true,horzTouchDrag:true,vertTouchDrag:true},handleScale:{mouseWheel:true,pinch:true,axisPressedMouseMove:{time:true,price:true},axisDoubleClickReset:{time:true,price:true}},localization:{locale:'es',timeFormatter:(t:import('lightweight-charts').Time)=>new Date(Number(t)*1000).toLocaleString('es',{timeZone:'America/New_York'})}});
 candles=chart.addSeries(CandlestickSeries,{upColor:'#17d6a2',downColor:'#ff6273',borderVisible:false,wickUpColor:'#17d6a2',wickDownColor:'#ff6273',priceLineColor:'#35d4df',autoscaleInfoProvider:(base:()=>AutoscaleInfo|null)=>{const info=base();return info?.priceRange?{...info,priceRange:gammaPriceRange(info.priceRange.minValue,info.priceRange.maxValue,latest)}:info;}});vol=chart.addSeries(LineSeries,{color:'#17e3be',lineWidth:2,title:'CVD · operaciones',priceFormat:{type:'volume'},lastValueVisible:true,priceLineVisible:true},1);chart.panes()[1].setHeight(160);vol.createPriceLine({price:0,color:"#8493a8",lineWidth:1,lineStyle:2,axisLabelVisible:true,title:"0"});vwap=chart.addSeries(LineSeries,{color:'#ff9800',lineWidth:2,lastValueVisible:false,priceLineVisible:false,crosshairMarkerVisible:false});
 let lastProfile=-Infinity;const sync=(at:number)=>{if(!host?.isConnected){cleanup();return;}if(at-lastProfile>=100){host.dataset.pricePaneHeight=String(chart?.panes()[0].getHeight()??host.clientHeight);const label=host.querySelector<HTMLElement>('.gl-cvd-label');if(label&&chart)label.style.top=(chart.panes()[0].getHeight()+7)+'px';profile();paintRouteProfiles();drawRoute();routeDrawings?.paint();paintBigTrades();paintExecutionAbsorption();paintAuctionBars();lastProfile=at;}scaleFrame=requestAnimationFrame(sync);};scaleFrame=requestAnimationFrame(sync);
 }
 flowRows=[];
 if(profileMetric==='bidask'){const dates=profileExpirations(s?.symbol===symbol?s.optionSnapshotStatus?.structuralEnrichment:undefined,profileHorizon,now);const strikes=s?.symbol===symbol?s.optionSnapshotStatus?.structuralEnrichment?.rows.map(r=>r.strike)??[]:[];const fresh=active&&s?.symbol===symbol&&s.optionSnapshotStatus.structuralEnrichment?.symbol===symbol&&s.options.phase==='streaming'&&s.publishedMs<=now&&now-s.publishedMs<=3000&&s.flow.asof_ms!=null&&s.flow.asof_ms<=now&&now-s.flow.asof_ms<=3000;flowRows=strikeBidAsk(s?.flow.bid_ask,dates,strikes,fresh);}
 retained=retainedContractProfile(s,symbol,now,profileHorizon);
 const chartHistory=extendedChartHistory(s,symbol,active,refresh,now);
 const daysBadge=root.querySelector<HTMLElement>('#chart-history-days');
 if(daysBadge)daysBadge.textContent=`${chartHistory?.sessions.length??0}/30 sesiones`;
 const prepared=gammaChartData(s,symbol,active,interval,now,chartHistory);
 latest=gammaLiveModel(s,symbol,active,now);
 const expiryDates=profileExpirations(s?.symbol===symbol?s.optionSnapshotStatus?.structuralEnrichment:undefined,profileHorizon,now);
 latest={...latest,contractProfile:chartContractReport(s,symbol,active,latest.h?{...latest.h,expirations:expiryDates}:undefined,now),contractProfileExpected:true};
 if(!prepared.currentOverlay)latest={...latest,contractProfile:undefined,rows:[],profileRows:[],profileGexRows:[],levels:latest.levels.map(l=>({...l,dominant:false,strike:null})),call:undefined,put:undefined};
 const bars=prepared.bars;
 chartAuctionReading=s?.symbol===symbol?s.auctionMap??undefined:undefined;chartAuctionSymbol=symbol;chartAuctionActive=active;
 executionAbsorptionRead=s?.cvd;executionAbsorptionSymbol=symbol;executionAbsorptionActive=active;executionAbsorptionBars=bars.map(b=>b.timestampMs);
 const observedBigTrades=active?chartBigTrades(s?.cvd,symbol,bigTradeMinimum,now):[];
 const recentTrades=tradeTape.observe(observedBigTrades,`${symbol}:${s?.startedMs}:${bigTradeMinimum}`,now);
 if(!recentTrades.some(p=>p.sequence===selectedBigTrade))selectedBigTrade=recentTrades[0]?.sequence;
 const tape=root.querySelector<HTMLElement>('.gl-big-trade-tape');if(tape){tape.hidden=!showBigTrades;const html=bigTradeCards(recentTrades,selectedBigTrade,showExecutionAbsorption?absorptionView.candidates:[])||'<small class="gl-big-trade-empty">En vivo · últimos 60 s · esperando un nuevo trade</small>';if(tape.innerHTML!==html){const scroll=tape.scrollLeft;tape.innerHTML=html;tape.scrollLeft=scroll;}tape.onclick=e=>{const card=(e.target as HTMLElement).closest<HTMLElement>('[data-trade-sequence]');if(card){selectedBigTrade=Number(card.dataset.tradeSequence);refresh();}};}
 if(tape){tape.scrollLeft=0;tape.onscroll=null;tape.onwheel=null;}
 const data=bars.map(b=>({time:b.timestampMs/1000 as import('lightweight-charts').UTCTimestamp,open:b.open,high:b.high,low:b.low,close:b.close}));
 const nextDataKey=JSON.stringify([symbol,interval,data,bars.map(b=>b.volume),prepared.vwap]);
 if(nextDataKey!==dataKey){dataKey=nextDataKey;
 // Data revisions preserve the user's time/price viewport.
 candles!.setData(data);
 vwap!.setData(prepared.vwap.map(b=>b.value!=null?{time:b.timestampMs/1000 as import('lightweight-charts').UTCTimestamp,value:b.value}:{time:b.timestampMs/1000 as import('lightweight-charts').UTCTimestamp}));
 }
 cvdChartEngine.observe(s,symbol,active,now);
 let cvdLabel=host!.querySelector<HTMLElement>('.gl-cvd-label');if(!cvdLabel){cvdLabel=document.createElement('span');cvdLabel.className='gl-cvd-label';host!.appendChild(cvdLabel);}
 const cvdRead=s?.cvd,cvdPeriod=s?.metrics.phase==='Premarket'?cvdRead?.premarket:cvdRead?.regular;
 const cvdStart=cvdPeriod?.sinceMs!=null?new Date(cvdPeriod.sinceMs).toLocaleTimeString('es',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit',hour12:false}):'—';
 cvdLabel.textContent='CVD · '+(active&&cvdRead?.symbol===symbol&&cvdRead.available&&cvdRead.at<=now&&now-cvdRead.at<=3000?'operaciones · cobertura '+(cvdPeriod?.coveragePct?.toFixed(0)??'—')+'%':'esperando datos vigentes')+' · desde '+cvdStart+' ET · arrastra la separación para ajustar'+(showExecutionAbsorption?' · '+absorptionView.label:'');
 cvdLabel.title=[absorptionView.detail,...absorptionView.candidates.map(absorptionCandidateDetail)].join(' · ');
 const cvdPoints=cvdChartEngine.series(bars,interval);vol!.setData(cvdPoints.map(p=>({...p,...(p.value!=null?{color:p.value<0?"#ff6273":"#17d6a2"}:{}),time:p.time as import('lightweight-charts').UTCTimestamp})));const cvdLast=cvdPoints.filter(p=>p.value!=null).at(-1);vol!.applyOptions({priceLineColor:cvdLast?.value!=null&&cvdLast.value<0?"#ff6273":"#17d6a2"});
 const quickLevels=!!root.closest('.quick-chart-levels');
 const snapshotStructure=latest.fresh?(latest.h?.structure.provisional??(latest.h?.structure.complete?latest.h.structure:undefined)):undefined;
 const quickM2=snapshotStructure?.selected.M2;
 const quickZero=latest.fresh&&latest.h?.exposure.scanComplete&&latest.h.exposure.gexComplete?latest.h.exposure.zeroGamma:null;
 const nextLineKey=JSON.stringify([symbol,quickLevels,quickM2,quickZero,latest.levels.map(l=>[l.id,l.strike,l.label]),latest.h?.structure.provisional!=null,latest.call?.vacuumLow,latest.call?.vacuumHigh,latest.put?.vacuumLow,latest.put?.vacuumHigh]);
 if(nextLineKey!==lineKey){lineKey=nextLineKey;
 for(const l of lines)candles!.removePriceLine(l);lines=[];
 const add=(p:number|null|undefined,title:string,color:string)=>{if(p!=null&&Number.isFinite(p)&&p>0)lines.push(candles!.createPriceLine({price:p,title,color,lineWidth:quickLevels?2:1,lineStyle:2,axisLabelVisible:true}));};
 for(const l of latest.levels.filter(l=>!quickLevels||l.id!=='liquidity'))add(l.strike,({call:'CW',put:'PW',liquidity:'Imán L',main:'Imán P'} as Record<string,string>)[l.id]??l.label,quickLevels?(l.id==='put'?'#ff6273':l.id==='call'?'#17d6a2':'#bc7dff'):(l.id==='put'?'#ff6273':l.id==='call'?'#35d4df':'#e8c66e'));
 if(quickLevels){add(quickZero,'ZERO GAMMA','#f0c869');}
 for(const [r,label] of [[latest.call,'Vacío superior'],[latest.put,'Vacío inferior']] as const){if(r?.vacuumLow!=null&&r.vacuumHigh!=null){for(const p of [r.vacuumLow,r.vacuumHigh])lines.push(candles!.createPriceLine({price:p,title:label==='Vacío superior'?'VS':'VI',color:'#7c91b6',lineWidth:1,lineStyle:3,axisLabelVisible:true}));}}
 }
 const alertSpot=readPriceNow(s,symbol,active,now).spot;
 chartAlarmPicker??=new ChartAlarmPicker(host!,candles!);chartAlarmPicker.bind(symbol);
 chartAlerts??=new ChartAlerts(candles!);chartAlerts.bind(root,symbol,prepared.currentOverlay&&alertSpot!=null?{price:alertSpot,at:s!.ticks[symbol].tradeMs!}:undefined,now);

 technicalOverlay(prepared,symbol,root.classList.contains('gl-route-chart'),s,active,now);
 const key=`${symbol}:${interval}`;if(chartKey!==key&&data.length){
  if(interval===1440||interval===240)chart!.timeScale().fitContent();
  else chart!.timeScale().setVisibleLogicalRange({from:Math.max(0,data.length-Math.ceil(390/interval)),to:data.length+5});
  chartKey=key;
 }
 if(root.classList.contains('gl-route-chart')){routeDrawings??=new RouteDrawingTools(host!,chart!,candles!);routeDrawings.bind(symbol,bars.map(b=>b.timestampMs));}else{routeDrawings?.dispose();routeDrawings=undefined;}
 profile();
}
let flowRows:StrikeFlow[]=[];
let retained:ReturnType<typeof retainedContractProfile>;
function profileRows(){
 const full=inventoryProfile(profileSymbol,profileNow());
 if(profileMetric==='oi'){if(!full)return [];try{return reportOiProfile(full).rows;}catch{return [];}}
 if(full&&profileMetric!=='bidask')return full.rows.map(r=>({strike:r.strike,call:profileMetric==='volume'?r.call.volume:profileMetric==='oi'?r.call.oi:r.call.gamma,put:profileMetric==='volume'?r.put.volume:profileMetric==='oi'?r.put.oi:r.put.gamma}));
 if(profileMetric==='bidask')return flowRows;
 if(!latest?.contractProfile&&retained)return retained.rows.map(r=>({strike:r.strike,call:profileMetric==='volume'?r.call.volume:profileMetric==='oi'?r.call.oi:r.call.gamma,put:profileMetric==='volume'?r.put.volume:profileMetric==='oi'?r.put.oi:r.put.gamma}));
 if(profileMetric!=='volume')return gammaContractRows(latest,profileMetric);
 return latest?.contractProfile?.rows.map(r=>({strike:r.strike,call:r.call.volume,put:r.put.volume}))??[];
}
function profile(){
 const svg=document.getElementById('gamma-live-profile');if(!svg||!host||!candles)return;
 const datesLabel=document.querySelector<HTMLElement>('.gl-profile-dates');
 const fullInventory=inventoryProfile(profileSymbol,profileNow());
 const included=profileMetric==='oi'?(fullInventory?.expirations??[]):fullInventory?.expirations??latest?.contractProfile?.expirations??retained?.expirations??[];
 const dateSummary=fullInventory?`${included.length} expiraciones · ${included[0]??'—'} a ${included.at(-1)??'—'}`:included.join(' / ');
 if(datesLabel){
  const html=included.length?included.map(d=>`<span class="gl-expiry-chip" title="Vencimiento ${esc(d)}">${esc(d.slice(8,10)+'/'+d.slice(5,7)+'/'+d.slice(2,4))}</span>`).join(''):'<span class="gl-expiry-empty">Sin vencimientos verificados</span>';
  if(datesLabel.innerHTML!==html)datesLabel.innerHTML=html;
  datesLabel.setAttribute('aria-label',`${included.length} vencimientos incluidos`);
 }
 const footer=document.querySelector<HTMLElement>('.gl-profile>small');
 if(footer&&profileMetric==='bidask')footer.textContent='Muestra suscrita · 5 min · — sin cobertura';
 if(footer&&profileMetric!=='bidask'&&latest?.contractProfile)footer.textContent='Perfil de opciones · '+included.join(' / ')+' · snapshot';
 if(footer&&profileMetric!=='bidask'&&!latest?.contractProfile&&retained)footer.textContent='ÚLTIMO SNAPSHOT · '+new Date(retained.at).toLocaleString('es',{timeZone:'America/New_York'})+' ET · '+retained.expirations.join(' / ')+' · solo visual';
 if(fullInventory&&footer)footer.textContent='CADENA COMPLETA · '+dateSummary+' · consulta REST · sin señal direccional';
 if(profileMetric==='oi'){
  if(footer){
   if(!fullInventory)footer.textContent='OI pendiente · actualiza la cadena · sin referencia independiente vigente';
   else try {const {map}=reportOiProfile(fullInventory);footer.textContent=`OI · ${map.expectedContracts} contratos estándar · cobertura ${map.coverage==null?'pendiente':(map.coverage*100).toFixed(1)+'%'} · captura ${new Date(map.observedMs).toLocaleString('es',{timeZone:'America/New_York'})} ET · fecha OI ${map.oiDate??'no suministrada'} · ${map.universeComplete?'referencia verificada':'referencia parcial'}`;}
   catch{footer.textContent='OI pendiente · referencia independiente no utilizable; actualiza la cadena';}
  }
 }
 const height=svg.clientHeight,rows=profileRows(),value=(r:typeof rows[number],call:boolean)=>call?r.call:r.put,peak=Math.max(1,...rows.map(r=>Math.max(value(r,true)??0,value(r,false)??0)));
 svg.setAttribute('viewBox',`0 0 390 ${height}`);svg.setAttribute('preserveAspectRatio','none');
 // The strike viewport owns its center and spacing; chart pan/zoom never supplies coordinates.
 if(profileCenter==null&&rows.length)profileCenter=routeSpot??latest?.levels.find(l=>l.id==='main')?.strike??retained?.spot??rows[Math.floor(rows.length/2)].strike;
 const positions=new Map(strikeProfileWindow(rows.map(r=>r.strike),profileCenter??0,height-65).map(r=>[r.strike,r.y]));
 const visible=rows.map(r=>({...r,y:positions.get(r.strike)})).filter(r=>r.y!=null&&r.y>Math.max(95,(document.querySelector<HTMLElement>('.gl-profile header')?.offsetHeight??80)+35)&&r.y<height-100);
 svg.setAttribute('aria-label',`Perfil ${profileMetric.toUpperCase()} por strike · escala independiente · rueda para recorrer`);
 const signature=JSON.stringify([profileMetric,profileCenter,height,peak,fullInventory?.at,included,visible.map(r=>[r.strike,r.y?.toFixed(2),r.call,r.put]),profileMetric==='bidask'?flowRows:[],latest?.levels.filter(l=>l.dominant).map(l=>l.strike)]);if(signature===profileKey&&svg.childElementCount>0)return;profileKey=signature;
 const gap=visible.length>1?Math.abs(visible[1].y!-visible[0].y!):20,every=Math.max(1,Math.ceil(24/Math.max(1,gap)));
 const dominant=latest?.levels.filter(l=>l.dominant).map(l=>l.strike)??[];
 const compact=(n:number|null)=>n==null?'—':new Intl.NumberFormat('es',{notation:'compact',maximumFractionDigits:1}).format(n);
 const html=profileMetric==='bidask'?bidAskSvg(visible.filter((_,i)=>i%every===0).map(r=>({...flowRows.find(f=>f.strike===r.strike)!,y:r.y!})),height,Math.max(80,(document.querySelector<HTMLElement>('.gl-profile header')?.offsetHeight??65)+20)):strikeHeatmap(visible.filter((_,i)=>i%every===0).map(r=>({...r,y:r.y!})),height,profileMetric,latest?.levels??[],profileMetric==='oi'?!!fullInventory&&(()=>{try{const {map}=reportOiProfile(fullInventory);return map.universeComplete&&map.coverage===1;}catch{return false;}})():profileHorizon==='DAY'&&!!latest?.contractProfile&&!!latest.complete,peak,Math.max(80,(document.querySelector<HTMLElement>('.gl-profile header')?.offsetHeight??65)+20));

 if(svg.innerHTML!==html)svg.innerHTML=html;
 const ticks=host.querySelector<HTMLElement>('.gl-dominant-ticks')??host.appendChild(Object.assign(document.createElement('div'),{className:'gl-dominant-ticks'}));
 const tickHtml=[...new Set(dominant)].map(k=>k!=null?candles!.priceToCoordinate(k):null).filter(y=>y!=null&&y>55&&y<height-35).map(y=>`<i style="top:${y}px"></i>`).join('');if(ticks.innerHTML!==tickHtml)ticks.innerHTML=tickHtml;
}









function paintRouteProfiles(){
 if(!host||!candles||!dualVisible)return;
 const root=host.closest('.gl-dual-visible');if(!root)return;
 const rows=latest?.fresh&&latest.h&&profileSnapshotFresh({phase:'ready',observedMs:latest.h.observedMs},Date.now())?latest.profileGexRows:[],spot=latest?.h?.exposure.spot;
 for(const [selector,gamma]of [['[data-route-gamma]',true],['[data-route-gex]',false]] as const){const svg=root.querySelector<SVGSVGElement>(selector);if(!svg)continue;svg.setAttribute('viewBox',`0 0 200 ${host.clientHeight}`);svg.setAttribute('preserveAspectRatio','none');const html=routeProfileSvg(rows,spot,chart?.panes()[0].getHeight()??host.clientHeight,p=>candles!.priceToCoordinate(p),gamma);if(svg.innerHTML!==html)svg.innerHTML=html;}
}

