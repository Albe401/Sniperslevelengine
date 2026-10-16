import {setWatchedNewsSymbols} from './news-watchlist';
import {twoMapsPanel,bindTwoMaps} from './two-maps';
import './two-maps.css';
import {bindLevelCardSpacing} from './level-card-spacing';
import {receiverHealth,transferConnection,type DataConnectionMode} from './data-connection';
import {startResearchWatchlistSync} from './research-watchlist';
import {startBackgroundOptionControl} from './option-background-control';
import {startBaselineReadings} from './arrival-baseline-reading';
import {marketNewsRevision} from './market-news';
import {nativeRevision} from './native-top-five';
import {cvdChartEngine} from './cvd-chart-engine';
import {gapPanel} from './gap-panel';
import {blockMemory} from './block-memory';
import {updateEtfCvd} from './etf-dealers';
import {momentumShadow} from './momentum-shadow';
import {allPriceAlerts,consumeBackgroundAlerts,type BackgroundAlert} from './chart-alerts';
import {continuousAudit} from './continuous-quick-audit';
import {quickReadingPanels} from './quick-reading-panels';
import {bindAuctionProfileNavigation} from './auction-profile-navigation';
import {quickChartLevels} from './quick-chart-levels';
import {structuralMapPreview} from './structural-map-preview';
import {directionLevels} from './direction-levels';
import {bindAnalystTargets} from './analyst-targets';
import {readDirectionScore} from './direction-scanner';
import {routePreview} from './route-preview';
import {readRouteLive,bindRouteJournal} from './route-live';
import {bindParallelRoute} from './route-parallel';

import {collectGammaLearning} from './gamma-learning';
import {gammaLivePage,bindGammaLive} from './gamma-live';
import {volatilityReadingPanel} from './volatility-reading';
import {neuroContextLine,type NeuroContext} from './neuro-context';
import {controlOutlook} from './control-outlook';
import {VisualCadence} from './visual-cadence';
import {spxReadingPanel,spxReadingFresh,type SpxReading} from './spx-reading';
import {type Sensitivity} from './theta-vega';
import {bindDrawingTools} from './chart-drawings';
import {assetPrints} from './asset-prints';
import {classificationNote} from './flow-classification';
import {flowFour} from './flow-four';
import {blockPressurePanel,selectBlockPeriod,type BlockReport} from './block-pressure';
import {quickDetails} from './quick-details';
import {mapHorizon,offSession} from './reading-display';
import {assetDataHealth} from './asset-data-health';
import {readPriceLevels,priceLevelsPanel} from './price-levels';
import {readPriceNow,priceNowPanel} from './price-now';
import {buildIntradayControl,controlPanel,ControlTracker} from './intraday-control';
import {selectedPrice,selectedPriceMarkup} from './selected-price';
import {updateLiveWorkspace} from './live-dom';
import {bundledAssets,assetResults,type AssetConfig} from './asset-watchlist';
import {topFivePanel,bindTopFive,scannerView} from './top-five';
import {sessionSignals} from './session-signals';
import {quickFlowPanel,type FlowWindow} from './quick-flow';
import {sessionCandles,type ChartHistory} from './candle-series';
import {gexMap} from './gex-map';
import {dayMap} from './day-map';
import {routeVolatility} from './route-design';
import {quickRangePanel,rangeContextPanel,rangeContextFresh,type RangeContext} from './range-context';
import {localVolumePanel,localVolumeFresh,type LocalVolume} from './local-volume';
import {paperChartMarks,brokerChartMarks} from './paper-chart';
import type {BotSession,BotState} from './paper-bot';
import {analystPanel,type AnalystContext} from './analyst';
import {cmePanel,type CmeStatus} from './cme';
import {spxUsable,spxSyntheticPanel,type SpxSynthetic} from './spx-synthetic';
import {decisionReviewPanel,type DecisionReview} from './decision-review';
import {optionLiquidityPanel, type OptionLiquidity} from './option-liquidity';
import {scannerPanel, type Scanner} from './scanner';
import {adjustedReadingPanel, type AdjustedReading} from './reading-adjusted';
import {wallPullPanel, type WallPull} from './wall-pull';
import {structuralPathPanel,type StructuralPath} from './structural-path';
import {wallFlowPanel, type WallFlow} from './wall-flow';
import { Channel, invoke, isTauri } from './snapshot-observation';
import { positionMapPanel, type PositionMap } from './position-map';
import { flowCoveragePanel, type ContractWindow } from './flow-coverage';
import { chart } from './chart';
import { dexChangePanel, dexFreshnessSignature, type DexChange } from './dex-change';
import { confirmationPanel, interpretationFresh, type MagnetConfirmation } from './confirmations';
import { structuralReadingPanel, type StructuralReading, type Alignment } from './reading';
import { dailyRangePanel, rangeFresh, type RangeSnapshot } from './daily-range';
import { aggregate, reconcileMinutes, escapeHtml as esc, price, type Candle } from './model';

interface Quality {duplicates:number;sequenceConflicts:number;unidentifiedTrades:number;futureEvents:number;outOfOrder:number;identityEvictions:number;latencySamples:number;p50Ms:number|null;p95Ms:number|null;p99Ms:number|null}
interface FeedState { quality:Quality; phase: string; detail: string; events: number; malformed: number; reconnects: number; channels: number; lastEventMs: number|null; lastReceivedMs: number|null }
interface Tick { symbol: string; price: number|null; bid: number|null; ask: number|null; tradeMs: number|null; quoteMs: number|null }
interface HistoryState {phase:string;detail:string;fromMs:number|null;throughMs:number|null;updatedMs:number|null;recoveredBars:number}
interface SessionMetrics {session:string;phase:string;vwap:number|null;rvol:number|null;volume:number|null;baselineSessions:number;asofMs:number|null}
interface SelectionState {structuralRequested:number;structuralResolved:number;spot:number|null;source:string;expiry:string;updatedMs:number|null;error:string|null}
interface FlowEvent {timestamp_ms:number;ticker:string;kind:string;price:number;size:number;premium:number|null;side:string;confidence:string;quote_state:string}
interface FlowSnapshot {heatmap?:import('./two-maps').FlowMapReport;bid_ask?:import("./strike-bid-ask").BidAskWindow[];asof_ms?:number;intraday_windows?:FlowWindow[];contract_windows:ContractWindow[];windows:FlowWindow[];recent:FlowEvent[];capacity_drops:number;rejected_trades:number;quote_max_age_ms:number}
interface OiChange {ticker:string;current:number|null;previous:number|null;change:number|null;changePct:number|null;previousObservedDate:string|null}
interface Exposure {callShareBounds?:[number,number]|null;deltaCapacityCoveragePct?:number|null;missingCallDexMax?:number|null;missingPutDexMax?:number|null;callDex?:number;putDex?:number;callDexPct?:number|null;putDexPct?:number|null;spot:number;eligible:number;unknownOi:number;gammaUsed:number;deltaUsed:number;scanUsed:number;gexComplete:boolean;dexComplete:boolean;scanComplete:boolean;netGex:number;netDex:number;zeroGamma:number|null;gammaRegime:string;dexSide:string;strikes:{strike:number;netGex:number;netDex:number}[]}
interface Structure {levelQuality?:import('./level-quality').LevelQuality[];strikeRows?:import('./gamma-live-model').GammaRow[];topContracts?:import('./top-strikes').ContractRanking;provisional?:{strikeRows?:import('./gamma-live-model').GammaRow[];coveredContracts:number;totalContracts:number;selected:Structure["selected"];callWall:Structure["callWall"];putWall:Structure["putWall"];magnet:number|null;mainMagnetScore?:number|null;liquidityMagnetScore?:number|null;routes:Structure["routes"]}|null;quickPressure?:number|null;reading:StructuralReading;complete:boolean;liquidityComplete:boolean;missingContracts:number;historyAvailable:boolean;gexAvailable:boolean;rows:number;selected:Record<string,number|null>;top:{strike:number;score:number;kind:string}[];callWall:{strike:number;score:number;ready:boolean;oi?:number;volume?:number;zoneOi?:number|null;zoneVolume?:number|null}|null;putWall:{strike:number;score:number;ready:boolean;oi?:number;volume?:number;zoneOi?:number|null;zoneVolume?:number|null}|null;magnet:number|null;mainMagnetScore?:number|null;liquidityMagnetScore?:number|null;routes:{pathLiquidity?:number|null;pathOpenness?:number|null;vacuumScore?:number|null;pullTarget?:number|null;vacuumLow?:number|null;vacuumHigh?:number|null;vacuumStatus?:string;direction:string;barrier:number|null;barrierScore?:number|null;pathState:string;target:number|null;extended:number|null;score:number|null;structuralReady:boolean}[]}
export interface Horizon {mode:string;label:string;expirations:string[];observedMs:number;referenceSource:string;exposure:Exposure;structure:Structure}
interface OptionSnapshotStatus extends RangeSnapshot {structuralEnrichment?:import("./etf-dealers").EtfReport|null;structuralAlignment:Alignment|null;phase:string;detail:string;observedMs:number|null;observedDate:string|null;previousObservedDate:string|null;contracts:number;oiMissing:number;greeksMissing:number;expirations:string[];changes:OiChange[];storageError:string|null;horizons:Horizon[];exposureError:string|null;dexChanges:Record<string,DexChange>}
interface OffLevel {level:number;prints:number;notional:number;concentration_pct:number;strength:string;strength_score:number;strength_data_sufficient:boolean}
export interface OffSummary {excluded_correction_rows?:number;date:string;print_count:number;live_count:number;history_count:number;history_complete:boolean;complete:boolean;total_notional:number;pressure_pct:number|null;pressure_coverage_pct:number|null;top_prints:{timestamp_ms:number;source:string;trf_id:number;conditions:number[];print:{price:number;shares:number;notional:number;side:string}}[];top_levels:OffLevel[]}
interface OffState {blockQuoteStatus?:string;analytics:{blocks?:BlockReport;session:OffSummary;history:OffSummary[];windows:{minutes:number;print_count:number;total_notional:number;complete:boolean;top_levels:OffLevel[]}[];last_live_ms:number|null;live_fresh:boolean;diagnostics:{duplicates:number;conflicts:number;unidentified:number;rejected:number;capacity_drops:number;corrections:number}};history:Record<string,{phase:string;detail:string;pages:number;stockTrades:number;offExchangeTrades:number;throughMs:number|null}>;queueDrops:number;error:string|null}
export interface LiveState {dataService?:{protocol:string;independent:boolean};auctionMap?:import('./auction-map-panel').AuctionReading|null;gapReport?:import('./gap-engine').GapReport;swingReport?:import("./swing-observation").SwingReport|null;qqq_report?:import('./qqq-components').ComponentRead|null;etfBlocks?:Record<string,import("./block-pressure").BlockReport>;etfFlow?:Record<string,import("./etf-enrichment").EtfFlow|null>;etfCvd?:Record<string,import("./cvd").CvdRead>;priceAlerts?:BackgroundAlert[];dailyAtr?:import('./intraday-atr').DailyAtr;cvd?:import('./cvd').CvdRead;treasury?:import('./treasury').Treasury;directionOi?:import('./direction-preparation').DirectionOi|null;flowExpiry?:string;levelDeltas?:import('./strike-absorption').StrikeDelta[];neuroContext?:NeuroContext;levelDynamics?:import('./level-dynamics').LevelDynamics;greekSupport?:{symbol:string;at:number;available:boolean;comparable:number;interpretation:string;model:string;contracts:unknown[]};thetaVega?:Sensitivity[];chartHistory?:ChartHistory;rangeContext?:RangeContext;localVolume?:LocalVolume;cme?:CmeStatus;spxSynthetic?:SpxSynthetic;spxReading?:SpxReading;decisionReview?:DecisionReview;structuralPaths:StructuralPath[];optionLiquidity:OptionLiquidity;scanner:Scanner;adjustedReadings:Record<string,AdjustedReading>;adjustedAlignment:Alignment|null;wallPull:Record<string,WallPull>;wallFlow:Record<string,WallFlow>;positionMaps:Record<string,PositionMap>;magnetConfirmations:Record<string,MagnetConfirmation>;offExchange:OffState;optionSnapshotStatus:OptionSnapshotStatus;flow:FlowSnapshot;journalRecords:number;overwrittenRecords:number; selection:SelectionState; symbol:string; stocks: FeedState; options: FeedState; ticks: Record<string, Tick>; bars: Record<string,Candle>; minutes:Record<string,Candle>; history:HistoryState; metrics:SessionMetrics; publishedMs: number; startedMs: number }
const intradayTracker=new ControlTracker();
let state: LiveState | undefined;
export function liveSnapshotForOvernight():{state:LiveState|undefined;active:boolean}{return {state,active};}
let analystContext:AnalystContext|undefined;
let analystBusy=false;
let analystError='';
let active = false;
let pending = false;
let connectionMode:DataConnectionMode='auto';
let error = '';
let exportMessage = '';
let exporting = false;
let channel: Channel<LiveState> | undefined;
let generation = 0;
let renderedSignature = "";
let barInterval = 2;
let selectedSymbol = 'QQQ';
let paperEntries = false;
let resumeLegacy = false;
let universe:Record<string,AssetConfig>={...bundledAssets};
let watchlistSlot:number|undefined;
let watchlistQuery='';
const defaultSlots=['QQQ','SPY','GOOGL','NVDA','AAPL'];
let assetSlots=[...defaultSlots];
let assetDrafts=[...assetSlots];
function saveAssetSlot(slot:number|undefined,symbol:string){
 if(slot==null)return;
 assetSlots[slot]=symbol;assetDrafts[slot]=symbol;setWatchedNewsSymbols(assetSlots);
 try{localStorage.setItem('snipers.asset-slots',JSON.stringify(assetSlots));}catch{/* Selection still works when storage is unavailable. */}
}
export async function initLive(){
 if(isTauri()){try{universe=await invoke('get_live_assets');}catch{error='No se pudo consultar el catálogo del motor. Se muestra el catálogo incluido.';}}
 try{const saved=JSON.parse(localStorage.getItem('snipers.asset-slots')??'null');
 if(Array.isArray(saved)&&saved.length===5)assetSlots=saved.map((symbol,i)=>typeof symbol==='string'&&universe[symbol]&&universe[symbol].kind!=='INDEX'?symbol:defaultSlots[i]);
 }catch{/* Keep defaults for invalid or unavailable storage. */}
 assetDrafts=[...assetSlots];setWatchedNewsSymbols(assetSlots);
 startResearchWatchlistSync();
  startBaselineReadings();
  startBackgroundOptionControl();
  document.addEventListener('snipers:baseline-probability',()=>{renderedSignature='';refresh();});
  document.addEventListener('snipers:auction-history',()=>{renderedSignature='';refresh();});
}
const value = (n: number|null|undefined) => n == null ? '—' : price(n);
export const age = (t: number|null|undefined, now = Date.now()) => t == null ? 'Sin datos' : t > now + 1000 ? 'Reloj adelantado' : `${Math.max(0,(now-t)/1000).toFixed(1)} s`;
const ageMarkup=(t:number|null|undefined)=>`<span data-event-age="${t??''}">${age(t)}</span>`;
const phases: Record<string,string> = {connecting:'Conectando',discovering:'Buscando contratos',authenticating:'Autenticando',subscribing:'Suscribiendo',waiting:'Esperando eventos',streaming:'Eventos recibidos',reconnecting:'Reconectando',denied:'Acceso detenido'};

function feedCard(title: string, f?: FeedState): string {
  const fresh = active && f?.phase === 'streaming' && f.lastEventMs != null && Date.now()-f.lastEventMs <= 15_000 && f.lastEventMs <= Date.now()+1000;
  return `<section class="panel connection-panel" data-health="${!active?'idle':fresh?'fresh':f?.phase==='denied'?'error':'waiting'}"><span class="eyebrow">MASSIVE · ${title.toUpperCase()} ADVANCED</span><h2>${!active?'Desconectado':fresh?'Datos recientes':phases[f?.phase??'']??'Iniciando'}</h2><p>${esc(!active?'Conecta para recibir eventos del mercado.':f?.detail||'Preparando conexión segura desde Rust.')}</p><div class="status-line"><span>Último evento · edad</span><strong>${ageMarkup(f?.lastEventMs)}</strong></div><div class="status-line"><span>Eventos de mercado aceptados</span><strong>${f?.events??0}</strong></div><div class="status-line"><span>Canales solicitados</span><strong>${f?.channels??0}</strong></div><div class="status-line"><span>Reconexiones / eventos inválidos</span><strong>${f?.reconnects??0} / ${f?.malformed??0}</strong></div></section>`;
}

function exposurePanel():string {
  const status=state?.optionSnapshotStatus;
  const compact=(n:number)=>Intl.NumberFormat('es',{notation:'compact',maximumFractionDigits:2}).format(n);
  return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">EXPOSICIÓN ESTIMADA · SNAPSHOT REST</span><h2>GEX, DEX y zero gamma</h2></div><span class="tag">${status?.phase==='ready'?'REFERENCIA GUARDADA':'PENDIENTE / NO ACTUALIZADO'}</span></div>
  <div class="table-wrap"><table><thead><tr><th>HORIZONTE</th><th class="numeric">GEX NETO · USD / 1%</th><th class="numeric">DEX NETO · EQUIV.</th><th class="numeric">ZERO GAMMA</th><th>COBERTURA G / D / SCAN</th></tr></thead><tbody>${status?.horizons?.length?status.horizons.map(h=>{const e=h.exposure;return `<tr><td>${esc(h.label)}<small class="exposure-dates">${esc(h.expirations.join(', ')||'Sin vencimientos')}</small></td><td class="numeric">${e.gammaUsed?compact(e.netGex):'—'}${e.gexComplete?'':' · parcial'}</td><td class="numeric">${e.deltaUsed?compact(e.netDex):'—'}${e.dexComplete?'':' · parcial'}</td><td class="numeric">${value(e.zeroGamma)}</td><td>${e.gammaUsed} / ${e.deltaUsed} / ${e.scanUsed} de ${e.eligible}<small class="exposure-dates">OI desconocido: ${e.unknownOi}</small></td></tr>`;}).join(''):'<tr><td colspan="5">Esperando cadena de opciones y metadata verificable.</td></tr>'}</tbody></table></div>
  <div class="panel-caption">${esc(status?.exposureError||'')}${status?.horizons?.[0]?`Referencia ${value(status.horizons[0].exposure.spot)} · ${esc(status.horizons[0].referenceSource)}. Observación ${new Date(status.horizons[0].observedMs).toLocaleString('es',{timeZone:'America/New_York'})} ET.<br>`:''}Modelo de exposición con call positivo / put negativo; no representa inventario real de dealers. Los totales parciales no autorizan un régimen ni una señal. Zero gamma exige datos completos para el scan y un cruce de signo.<br>Black-Scholes aproximado: tasa 4%, dividendo 0,5%, rango ±10% y 241 puntos; parámetros heredados, no tasas actuales. Convención de modelo: 16:00 ET; no es el horario contractual de negociación. El cálculo cambia al actualizar el snapshot; trades y quotes se reciben continuamente por WebSocket. “Próximos 4 vencimientos” conserva la regla vigente del antiguo FOUR_WEEKS.</div></section>`;
}

function offExchangePanel(showReading=false):string {
  const money=(n:number|null|undefined)=>n==null?'—':Intl.NumberFormat('es',{notation:'compact',maximumFractionDigits:2}).format(n===0?0:n);
  const off=state?.offExchange, data=off?.analytics, session=offSession(data?.session,data?.history,offSessionDate);
  const historical=session!=null&&session.date!==data?.session.date;
  const sessions=[...(data?.session?[data.session]:[]),...(data?.history??[])].sort((a,b)=>b.date.localeCompare(a.date));
  const levels=session?.top_levels??[];
  const downloads=Object.entries(off?.history??{}).filter(([,h])=>h.phase==='loading'||h.phase==='failed');
  const quote=selectedPrice(selectedSymbol,state,active);
  return `${showReading?assetPrints(state?.symbol===selectedSymbol?session:undefined,quote.value,quote.label + (quote.at == null ? "" : " · " + new Date(quote.at).toLocaleString("es", {timeZone:"America/New_York"}) + " ET")):''}<section class="panel"><header class="panel-head"><div><span class="eyebrow">STOCKS · FUERA DE BOLSA / TRF</span><h2>Concentración de operaciones</h2></div><span class="tag">${historical?'SESIÓN HISTÓRICA':data?.live_fresh?'PRINT RECIENTE':'SIN PRINT RECIENTE'}</span></header>
  <label>Sesión <select data-off-session aria-label="Sesión de operaciones fuera de bolsa"><option value="" ${offSessionDate===''?'selected':''}>Última sesión con datos</option>${sessions.map(s=>`<option value="${esc(s.date)}" ${offSessionDate===s.date?'selected':''}>${esc(s.date)} · ${s.print_count} prints</option>`).join('')}</select></label>
  <div class="panel-caption">${historical?'Histórico; no representa actividad en vivo. ':''}${esc(session?.date||'Esperando conexión')} · ${session?.print_count??0} prints · ${session?.live_count??0} live / ${session?.history_count??0} históricos · nocional ${money(session?.total_notional)} · ${session?.complete?'historial conciliado':'cobertura sin certificar'}. Último print live: ${ageMarkup(data?.last_live_ms)}.</div>
  <div class="table-wrap"><table><thead><tr><th>NIVEL</th><th>PRINTS</th><th>NOCIONAL USD</th><th>CONCENTRACIÓN</th><th>FUERZA HEURÍSTICA</th></tr></thead><tbody>${levels.map(l=>`<tr><td>${value(l.level)}</td><td>${l.prints}</td><td>${money(l.notional)}</td><td>${value(l.concentration_pct)} %</td><td>${esc(l.strength)} · ${value(l.strength_score)} / 100</td></tr>`).join('')||'<tr><td colspan="5" class="empty-state">Sin prints de la sesión seleccionada; revisa el estado de descarga debajo.</td></tr>'}</tbody></table></div>
  ${downloads.length?`<p class="panel-caption" role="status">${downloads.map(([date,h])=>`${esc(date)}: ${h.phase==='failed'?'Error':'Descargando historial'} · ${h.pages} páginas · ${h.offExchangeTrades} operaciones TRF. ${esc(h.detail)}`).join('<br>')}</p>`:''}
  <div class="panel-caption">Presión inferida ${value(session?.pressure_pct)} % · cobertura direccional ${value(session?.pressure_coverage_pct)} %. El histórico conserva lado desconocido; no se reconstruye con el NBBO actual.</div>
  <div class="table-wrap"><table><thead><tr><th>VENTANA LIVE + REST</th><th>PRINTS ≥ 100.000 USD</th><th>NOCIONAL</th><th>COBERTURA</th></tr></thead><tbody>${(historical?[]:data?.windows??[]).map(w=>`<tr><td>${w.minutes}m</td><td>${w.print_count}</td><td>${money(w.total_notional)}</td><td>${w.complete?'Conciliada':'Parcial / pendiente'}</td></tr>`).join('')}</tbody></table></div>
  <details class="structure-details" data-horizon-details="off-prints"><summary>Mayores prints de la sesión</summary><div class="table-wrap"><table><thead><tr><th>HORA ET</th><th>PRECIO / ACCIONES</th><th>NOCIONAL</th><th>ORIGEN / LADO INFERIDO</th><th>TRF / CONDICIONES</th></tr></thead><tbody>${(session?.top_prints??[]).map(p=>`<tr><td>${new Date(p.timestamp_ms).toLocaleTimeString('es',{timeZone:'America/New_York'})}</td><td>${value(p.print.price)} / ${value(p.print.shares)}</td><td>${money(p.print.notional)}</td><td>${esc(p.source)} / ${esc(p.print.side)}</td><td>${p.trf_id} / ${p.conditions.join(', ')||'—'}</td></tr>`).join('')||'<tr><td colspan="5" class="empty-state">Sin prints de esta sesión.</td></tr>'}</tbody></table></div></details>
  <details class="structure-details" data-horizon-details="off-history"><summary>Cinco sesiones anteriores y estado de descarga</summary><div class="table-wrap"><table><thead><tr><th>FECHA ET</th><th>ESTADO</th><th>PÁGINAS / TRADES</th><th>PRINTS / FILAS EXCLUIDAS</th><th>IMÁN HISTÓRICO</th></tr></thead><tbody>${Object.entries(off?.history??{}).sort(([a],[b])=>b.localeCompare(a)).map(([date,h])=>{const historic=data?.history.find(d=>d.date===date)??(session?.date===date?session:undefined);return `<tr><td>${esc(date)}</td><td>${esc(({ready:"Completo",loading:"Descargando",failed:"Error"} as Record<string,string>)[h.phase]??h.phase)}<small class="exposure-dates">${esc(h.detail)}</small></td><td>${h.pages} / ${h.stockTrades}</td><td>${historic?.print_count??'—'} / ${historic?.excluded_correction_rows??'—'}</td><td>${value(historic?.top_levels[0]?.level)}${historic?' · referencia':''}</td></tr>`;}).join('')||'<tr><td colspan="5" class="empty-state">Se consulta tras aceptar la suscripción Stocks.</td></tr>'}</tbody></table></div></details>
  <div class="panel-caption">Exchange 4 con TRF presente identifica actividad fuera de bolsa; incluye operaciones ATS y no ATS. El imán es el nivel de mayor nocional, no una predicción. BUY/SELL requiere un NBBO válido anterior a la ejecución y de hasta 2 s; NBBO bloqueado/cruzado conserva lado desconocido. El tamaño decimal incluye fracciones de acción. Las cancelaciones posteriores no llegan por este WebSocket: los resultados son provisionales hasta su reconciliación REST. Filas corregidas excluidas en la última revisión de sesión: ${session?.excluded_correction_rows??0}.<br>Duplicados ${data?.diagnostics.duplicates??0} · conflictos ${data?.diagnostics.conflicts??0} · sin identidad ${data?.diagnostics.unidentified??0} · correcciones no resueltas ${data?.diagnostics.corrections??0} · rechazos ${data?.diagnostics.rejected??0} · descartes por capacidad ${(data?.diagnostics.capacity_drops??0)+(off?.queueDrops??0)}.${off?.error?`<br>${esc(off.error)}`:''}</div></section>`;
}

function structurePanel():string {
  const horizons=state?.optionSnapshotStatus?.horizons??[];
  const wall=(w:Structure['callWall'])=>w?`${value(w.strike)} · ${value(w.score)} / 100`:'—';
  return `<section class="panel"><div class="panel-head"><div><span class="eyebrow">CONCENTRACIÓN ESTRUCTURAL · SNAPSHOT</span><h2>Niveles y liquidez</h2></div><span class="tag">CONTEXTO · SIN ENTRADAS</span></div><div class="table-wrap"><table><thead><tr><th>HORIZONTE</th><th class="numeric">R1 / S1 / M1</th><th class="numeric">CALL WALL · FUERZA</th><th class="numeric">PUT WALL · FUERZA</th><th>COBERTURA</th></tr></thead><tbody>${horizons.length?horizons.map(h=>{const s=h.structure;return `<tr><td>${esc(h.label)}</td><td class="numeric">${value(s.selected.R1)} / ${value(s.selected.S1)} / ${value(s.selected.M1)}</td><td class="numeric">${wall(s.callWall)}</td><td class="numeric">${wall(s.putWall)}</td><td>${s.rows} strikes · ${s.complete?'OI/volumen disponibles':'OI/volumen incompletos'}<small class="exposure-dates">${s.liquidityComplete?'Liquidez calculada':'Liquidez sin certificar'} · ${s.historyAvailable?'con base OI':'sin base OI'} · ${s.gexAvailable?'con GEX':'pesos sin GEX'}</small></td></tr>`;}).join(''):'<tr><td colspan="5">Esperando información para calcular niveles.</td></tr>'}</tbody></table></div>
  ${horizons.map(h=>`<details class="structure-details" data-horizon-details="${esc(h.mode)}"><summary>${esc(h.label)} · ranking y trayectos</summary><div class="table-wrap"><table><thead><tr><th>STRIKE</th><th>CLASIFICACIÓN</th><th class="numeric">PUNTUACIÓN / 100</th></tr></thead><tbody>${h.structure.top.map(r=>`<tr><td>${value(r.strike)}</td><td>${esc(r.kind)}</td><td class="numeric">${value(r.score)}</td></tr>`).join('')}</tbody></table></div><div class="panel-caption">${h.structure.routes.map(r=>`${esc(r.direction)}: ${esc(r.pathState)} · barrera ${value(r.barrier)} · objetivo ${value(r.target)} · extendido ${value(r.extended)} · pull ${value(r.score)} / 100`).join('<br>')}</div></details>`).join('')}
  <div class="panel-caption">Puntuaciones de concentración, no probabilidades. Volumen acumulado del snapshot; no representa volumen entrante en vivo. Los pesos se redistribuyen si falta historial OI o GEX completo. Los rankings parciales son provisionales; datos OI/volumen incompletos no certifican niveles, y griegas incompletas no certifican liquidez. Objetivos y muros no autorizan operaciones.</div></section>`;
}

let offSessionDate='';
let gexHorizon='DAY';
let gexHeight=560,gexPriceRange:[number,number]|undefined;
let brokerMarks:ReturnType<typeof brokerChartMarks>=[],brokerMarksError='',brokerMarksPolling=false,brokerMarksAt=0;
async function loadBrokerMarks(){
 const view=document.querySelector('#live-workspace')?.getAttribute('data-live-view');
 if(!isTauri()||document.hidden||!['gex','monitor','levels'].includes(view??'')||brokerMarksPolling||Date.now()-brokerMarksAt<2000)return;
 brokerMarksPolling=true;brokerMarksAt=Date.now();
 try{
  const [history,current]=await Promise.all([invoke<BotSession[]>('get_paper_bot_history'),invoke<BotState>('get_paper_bot_state')]);
  brokerMarks=brokerChartMarks(history,current.session,selectedSymbol);brokerMarksError='';
 }catch{brokerMarksError='No se pudo actualizar el registro de flechas de Alpaca Paper.';}
 finally{brokerMarksPolling=false;refresh();}
}
const chartMarks=()=>[...paperChartMarks(state?.decisionReview,selectedSymbol),...brokerMarks];
let gexPanOffset=0;
let gexDate='',gexVisible=48,gexBefore:number|undefined,gexExpanded=false,gexDragging=false;
let gexCandles:ReturnType<typeof sessionCandles>=[];
export type LiveView='gamma-live'|'top-five'|'gex'|'monitor'|'flow'|'maps'|'levels'|'quick'|'detailed'|'asset'|'spx';
const liveGroup=(group:LiveView,html:string)=>`<div data-live-group="${group}">${html}</div>`;
function watchlistMarkup():string {
 return watchlistSlot==null?'':`<section class="asset-watchlist" aria-label="Watchlist completa"><header><strong data-watchlist-heading>Cambiar activo ${watchlistSlot+1}</strong><button type="button" data-watchlist-close>Cerrar</button></header><label for="watchlist-query">Símbolo, nombre o sector</label><input id="watchlist-query" type="search" value="${esc(watchlistQuery)}" placeholder="Ej. AMD, Microsoft" autocomplete="off"/><div data-watchlist-results>${assetResults(universe,watchlistQuery)}</div></section>`;
}
function showWatchlist(slot:number,query:string,focus=false):void {
 watchlistSlot=slot;watchlistQuery=query;
 const host=document.querySelector('[data-watchlist-host]');
 if(!host)return;
 if(!host.querySelector('.asset-watchlist'))host.innerHTML=watchlistMarkup();
 else {
  host.querySelector('[data-watchlist-heading]')!.textContent=`Cambiar activo ${slot+1}`;
  host.querySelector<HTMLInputElement>('#watchlist-query')!.value=query;
  host.querySelector('[data-watchlist-results]')!.innerHTML=assetResults(universe,query);
 }
 bindWatchlist();
 if(focus)host.querySelector<HTMLInputElement>('#watchlist-query')?.focus({preventScroll:true});
}
function bindWatchlist():void {
 const bindResults=()=>document.querySelectorAll<HTMLButtonElement>('[data-watchlist-symbol]').forEach(b=>b.onclick=()=>{void selectLiveAsset(b.dataset.watchlistSymbol!,watchlistSlot);});
 document.querySelectorAll<HTMLButtonElement>('[data-watchlist-open]').forEach(b=>b.onclick=()=>showWatchlist(Number(b.dataset.watchlistOpen),'',true));
 const close=document.querySelector<HTMLButtonElement>('[data-watchlist-close]');
 if(close)close.onclick=()=>{const slot=watchlistSlot;watchlistSlot=undefined;document.querySelector('.asset-watchlist')?.remove();document.querySelector<HTMLButtonElement>(`[data-watchlist-open="${slot}"]`)?.focus({preventScroll:true});};
 const search=document.querySelector<HTMLInputElement>('#watchlist-query');
 if(search){search.oninput=()=>{watchlistQuery=search.value;document.querySelector('[data-watchlist-results]')!.innerHTML=assetResults(universe,watchlistQuery);bindResults();};search.onkeydown=e=>{if(e.key==='Escape')close?.click();if(e.key==='ArrowDown'){e.preventDefault();document.querySelector<HTMLButtonElement>('[data-watchlist-symbol]')?.focus({preventScroll:true});}};}
 bindResults();
}
export function massiveConnectionButton():string {
  const independent=active?state?.dataService?.independent===true:connectionMode!=='direct';
  const health=active?receiverHealth(independent,state?.publishedMs,state?.stocks.phase==='disconnected',Date.now()):'';
  return `<button class="massive-connection-button ${active?'is-connected':''}" data-live-action ${pending||!isTauri()?'disabled':''} title="${esc(error||'Conexión compartida de datos Stocks + Options')}">${pending?'Espera…':active?(independent?'Desconectar vista':'Desconectar Massive'):'Conectar Massive'}</button><button class="massive-connection-button" data-switch-connection="${independent?'direct':'receiver'}" ${pending||!isTauri()?'disabled':''} title="${independent?'Detiene el receptor y conecta Massive directamente en Snipers. Al cerrar Snipers se detendrá la recepción.':'Desconecta la conexión directa y vuelve al receptor independiente.'}">${independent?'Conexión directa':'Usar receptor'}</button>${health?`<span role="status">${esc(health)}</span>`:''}<span class="massive-connection-error" role="status" ${error?'':'hidden'}>${esc(error)}</span>`;
}
function refreshMassiveConnection():void {
 const host=document.querySelector<HTMLElement>('[data-massive-connection]');
 if(!host)return;
 const signature=JSON.stringify([active,pending,error,isTauri(),connectionMode,state?.dataService?.independent===true,receiverHealth(state?.dataService?.independent===true,state?.publishedMs,state?.stocks.phase==='disconnected',Date.now())]);
 if(host.dataset.massiveState!==signature){host.dataset.massiveState=signature;host.innerHTML=massiveConnectionButton();bindMassiveConnection();}
}
function assetNavigation(showConnection=false,editable=false):string {
 const locked=pending;
 if(!editable)return `<div class="live-asset-navigation live-asset-context"><div data-asset-status><span class="tag">${esc(selectedSymbol)} · ${pending?'CAMBIANDO…':active?'CONECTADO':'SIN CONEXIÓN'}</span><small>Activo compartido · selección en Lectura rápida</small>${showConnection?`<button class="primary-button" data-live-action ${pending||!isTauri()?'disabled':''}>${pending?'Espera…':active?'Desconectar':'Conectar Massive'}</button>`:''}${error?`<p role="status">${esc(error)}</p>`:''}</div></div>`;
 return `<div class="live-asset-navigation"><div class="live-asset-slots" role="group" aria-label="Cinco activos editables">${assetSlots.map((symbol,i)=>`<div class="live-asset-slot"><button class="slot-select" data-live-asset="${esc(symbol)}" aria-pressed="${selectedSymbol===symbol}" ${locked?'disabled':''}>${esc(symbol)}</button><form data-asset-slot="${i}"><label for="live-asset-search-${i}">Activo ${i+1}</label><div><input id="live-asset-search-${i}" data-slot-input="${i}" name="symbol" value="${esc(assetDrafts[i])}" autocomplete="off" spellcheck="false" ${locked?'disabled':''}/><button type="submit" ${locked?'disabled':''}>Ver activo</button></div></form></div>`).join('')}</div><div data-watchlist-host>${watchlistMarkup()}</div>${showConnection||error?`<div data-asset-status>${showConnection?`<button class="primary-button" data-live-action ${pending||!isTauri()?'disabled':''}>${pending?'Espera…':active?'Desconectar':'Conectar Massive'}</button>`:''}${error?`<p role="status">${esc(error)}</p>`:''}</div>`:''}</div>`;
}
export async function selectLiveAsset(raw:string,slot?:number):Promise<void> {
 const symbol=raw.trim().toUpperCase();
 if(pending)return;
 if(!universe[symbol]||universe[symbol].kind==='INDEX'){error='Selecciona una acción o ETF disponible en la búsqueda.';refresh();return;}
 if(symbol===selectedSymbol){saveAssetSlot(slot,symbol);watchlistSlot=undefined;error='';renderedSignature='';refresh();return;}
 if(active&&(paperEntries||resumeLegacy)){error='Desconecta el simulador en Mercado en vivo antes de cambiar de activo.';refresh();return;}
 watchlistSlot=undefined;
 const reconnect=active;
 pending=true;error='';refresh();
 try {
  if(active){await invoke('stop_live');active=false;}
  blockMemory.remember(state,Date.now(),true);generation++;channel=undefined;state=undefined;gexPriceRange=undefined;gexDate='';gexBefore=undefined;gexPanOffset=0;selectedSymbol=symbol;brokerMarks=[];brokerMarksAt=0;saveAssetSlot(slot,symbol);
  paperEntries=false;resumeLegacy=false;analystContext=undefined;analystError='';exportMessage='';
  if(reconnect&&isTauri()){
   const token=generation;channel=new Channel<LiveState>();
   channel.onmessage=payload=>{if(token===generation&&payload.symbol===selectedSymbol){state=payload;cvdChartEngine.observe(payload,selectedSymbol,active,Date.now());blockMemory.remember(payload);updateEtfCvd(payload.etfCvd,payload.stocks.phase==="streaming",payload);consumeBackgroundAlerts(payload.priceAlerts??[]);readRouteLive(payload,selectedSymbol,active);collectGammaLearning(payload);refresh();}};
   await invoke('start_live',{connectionMode,priceAlerts:allPriceAlerts(),symbol,entriesEnabled:false,resumeLegacy:false,onMessage:channel});active=true;
  }
 }catch(e){error=String(e);}finally{pending=false;refresh();}
}
/** Same direction reader used by Quick; no additional feed or alternate weighting. */
export function sharedNeuroDirection(symbol:string,now=Date.now()) {
 const linked=active&&state?.symbol===symbol;
 return {at:now,symbol,...readDirectionScore(linked?state:undefined,buildIntradayControl(linked?state:undefined,symbol,!!linked,now),!!linked,now)};
}
export function livePage(view:LiveView='monitor'): string {
  if(view==='maps')return `<section id="live-workspace" data-live-view="maps">${assetNavigation()}${twoMapsPanel(state,selectedSymbol,active)}</section>`;
  if(view==='gamma-live')return `<section id="live-workspace" data-live-view="gamma-live">${assetNavigation(false,true)}${gammaLivePage(state,selectedSymbol,active)}</section>`;
  if(view==='spx') return `<section id="live-workspace" data-live-view="spx"><div class="panel-head"><div><span class="eyebrow">ÍNDICE · OPCIONES PROPIAS</span><h2>Lectura SPX</h2><p>Comparte la conexión Options del monitor. Las cinco acciones conservan su selección.</p></div></div>${error?`<p role="alert">${esc(error)}</p>`:''}${!isTauri()?'<p class="migration-notice">Abre la aplicación Windows para conectar.</p>':''}${spxSyntheticPanel(state?.spxSynthetic,active&&state?.options.phase==='streaming',state?.publishedMs)}${spxReadingPanel(state?.spxReading,state?.spxSynthetic,active&&state?.options.phase==='streaming',state?.publishedMs)}</section>`;
  if(view==='top-five') return `<section id="live-workspace" data-live-view="top-five">${topFivePanel(state?.scanner,active,state?.publishedMs)}</section>`;
  if(view==='gex'){
   const snapshot=state?.optionSnapshotStatus;
   const fresh=active&&snapshot?.phase==='ready'&&rangeFresh(snapshot)&&interpretationFresh(state?.publishedMs);
   const horizons=snapshot?.horizons??[];
   if(horizons.length&&!horizons.some(h=>h.mode===gexHorizon))gexHorizon=horizons[0].mode;
   const h=horizons.find(h=>h.mode===gexHorizon);
   const source=active?sessionCandles(state?.chartHistory,Object.values(state?.bars??{}),barInterval):[];
   gexCandles=source;
   const dates=[...new Set(source.map(c=>c.sessionDate))].reverse();
   if(gexDate&&!dates.includes(gexDate))gexDate='';
   return `<section id="live-workspace" data-live-view="gex" class="${gexExpanded?'gex-expanded':''}">${assetNavigation()}<div class="gex-controls"><label>Horizonte <select data-gex-horizon>${(horizons.length?horizons:[{mode:'DAY',label:'Hoy · DAY'}]).map(h=>`<option value="${esc(h.mode)}" ${gexHorizon===h.mode?'selected':''}>${esc(h.label)}</option>`).join('')}</select></label><label>Sesión <select data-gex-date><option value="">Última sesión disponible</option>${dates.map(d=>`<option ${gexDate===d?'selected':''}>${d}</option>`).join('')}</select></label><div class="segmented">${[1,2,5,15].map(n=>`<button data-live-interval="${n}" aria-pressed="${barInterval===n}" class="${barInterval===n?'active':''}">${n}m</button>`).join('')}</div><div class="gex-navigation">${[['older','←'],['zoom-out','−'],['zoom-in','+'],['newer','→'],['latest','Recientes'],['expand',gexExpanded?'Reducir':'Ampliar']].map(([action,label])=>`<button data-gex-action="${action}" aria-label="${({older:'Velas anteriores',newer:'Velas siguientes','zoom-in':'Acercar velas','zoom-out':'Alejar velas'} as Record<string,string>)[action]??label}">${label}</button>`).join('')}</div></div><p class="panel-caption">Historial: ${esc(state?.history.phase??'pendiente')} · ${esc(state?.history.detail??'Conecta en Mercado en vivo')} · ${dates.length} sesiones disponibles. Rueda: zoom temporal · rueda sobre precios o Mayús: escala vertical · arrastra el eje de precios para estirar.</p>${gexMap(selectedSymbol,h,!!fresh,source,barInterval,Date.now(),{prepared:true,date:gexDate,visible:gexVisible,before:gexBefore,panOffset:gexPanOffset,marks:chartMarks(),height:gexHeight,priceRange:gexPriceRange})}<p class="panel-caption">▲ E: entrada ejecutada · ▼ S: salida ejecutada · SIM: simulación interna. ${esc(brokerMarksError)}</p></section>`;
  }
  if(view==='quick'||view==='detailed'||view==='asset') {
    const snapshot=state?.optionSnapshotStatus;
    const fresh=active&&snapshot?.phase==='ready'&&rangeFresh(snapshot);
    const header=assetNavigation()+`<p class="panel-caption" role="status">${!isTauri()?'Vista web: la conexión a Massive funciona desde la aplicación de Windows.':!active?'Sin conexión: conecta Massive en Mercado en vivo para cargar este activo.':`Historial: ${esc(state?.history.detail??'Cargando')} · Opciones: ${esc(state?.optionSnapshotStatus?.phase??'pendiente')} · Stocks: ${esc(state?.stocks.detail??'Esperando eventos')}`}</p>`;
    const details=(title:string,html:string)=>`<details class="panel structure-details" data-horizon-details="reading-${title}"><summary>${title}</summary>${html}</details>`;
    if(view==='quick')return `<section id="live-workspace" class="visual-reading" data-live-view="quick">${assetNavigation(false,true)}${quickChartLevels(state,selectedSymbol,active,Date.now(),intradayTracker.update(buildIntradayControl(state,selectedSymbol,active),Date.now()))}</section>`;
    if(view==='detailed') {
      const readingNow=Date.now();
      const reading=intradayTracker.update(buildIntradayControl(state,selectedSymbol,active,readingNow),readingNow);
      const asset=assetNavigation(false,true),delayed=active&&!interpretationFresh(state?.publishedMs)?'<p class="panel-caption" role="status">Publicación retrasada · niveles del snapshot vigente; precio, flujo y confirmaciones requieren datos recientes.</p>':'';
      return `<section id="live-workspace" class="visual-reading" data-live-view="detailed">${asset}${delayed}${quickReadingPanels(state,selectedSymbol,active,reading,readingNow)}</section>`;
    }
    return `<section id="live-workspace" class="visual-reading" data-live-view="${view}">
    
    ${offExchangePanel(true)}
    ${header}${priceNowPanel(readPriceNow(state,selectedSymbol,active))}${priceLevelsPanel(readPriceLevels(state,selectedSymbol,active,readPriceNow(state,selectedSymbol,active)))}${assetDataHealth(state)}${gapPanel(state,selectedSymbol,active)}
    ${details('Escenarios CALL y PUT',wallPullPanel(state?.wallPull,active,state?.publishedMs,state?.stocks?.lastEventMs,state?.options?.lastEventMs)+positionMapPanel(state?.positionMaps,active,state?.publishedMs))}
    ${details('Lectura integrada y confirmaciones',adjustedReadingPanel(state?.adjustedReadings,state?.adjustedAlignment,active,state?.publishedMs)+neuroContextLine(state?.neuroContext,selectedSymbol,active)+confirmationPanel(state?.magnetConfirmations,active,state?.publishedMs))}
    ${details('GEX, DEX y niveles detallados',exposurePanel()+structurePanel())}</section>`;
  }
  const stock = state?.ticks[selectedSymbol];
  const canonical = Object.values(state?.minutes??{});
  const merged = reconcileMinutes(canonical,Object.values(state?.bars??{}));
  const bars = aggregate(merged,barInterval).slice(-180);
  const options = Object.values(state?.ticks??{}).filter(t=>t.symbol.startsWith('O:'));
  return `<section id="live-workspace" data-live-view="${view}">${assetNavigation()}<div class="live-toolbar"><div><span class="eyebrow">RECEPCIÓN REAL · ${esc(selectedSymbol)}</span><p>Stocks + Options · conexión persistente</p><label class="live-symbol"><input type="checkbox" data-paper-entries ${paperEntries?'checked':''} ${active||pending||!isTauri()||selectedSymbol!=='QQQ'?'disabled':''}> Permitir entradas simuladas · QQQ</label><label class="live-symbol"><input type="checkbox" data-resume-legacy ${resumeLegacy?'checked':''} ${active||pending||!isTauri()?'disabled':''}> Reanudar posiciones importadas · programa anterior detenido</label></div></div>
  ${!active?'<p class="migration-notice">Cierra la conexión del programa Python antes de conectar aquí: Massive permite por defecto un WebSocket por mercado.</p>':''}
  ${!isTauri()?'<p class="migration-notice">Abre la aplicación Windows para usar la conexión local.</p>':''}
  ${error?`<p role="alert" class="migration-notice">${esc(error)}</p>`:''}
  <div class="connection-grid">${feedCard('Stocks',state?.stocks)}${feedCard('Options',state?.options)}</div>
  <div class="metrics"><div class="metric"><span>VWAP de sesión RTH</span><div>${value(state?.metrics?.vwap)}</div><p>VWAP del proveedor · requiere cobertura completa</p></div><div class="metric"><span>RVOL a la misma hora</span><div>${value(state?.metrics?.rvol)}</div><p>${state?.metrics?.baselineSessions??0} / 5 sesiones históricas válidas</p></div><div class="metric"><span>Volumen RTH verificado</span><div>${state?.metrics?.volume==null?'—':Intl.NumberFormat('es',{notation:'compact'}).format(state.metrics.volume)}</div><p>Requiere 30 minutos y ≥95% de cobertura</p></div><div class="metric"><span>${esc(state?.metrics?.session||'Sesión')}</span><div class="context-value">${esc(state?.metrics?.phase||'Sin datos')}</div><p>${state?.metrics?.asofMs?`Cálculo hasta ${new Date(state.metrics.asofMs).toLocaleTimeString('es',{timeZone:'America/New_York',hour:'2-digit',minute:'2-digit'})} ET`:'No se estiman valores ausentes'}</p></div></div>
  <section class="panel live-price-panel" data-live-group="levels"><div class="panel-head"><div><span class="eyebrow">${esc(selectedSymbol)} · ÚLTIMOS DATOS RECIBIDOS</span><h2>${value(stock?.price)} <span class="muted">USD</span></h2></div><div class="live-quote"><span>Bid ${value(stock?.bid)} · Ask ${value(stock?.ask)}</span><small>Trade: ${ageMarkup(stock?.tradeMs)} · Quote: ${ageMarkup(stock?.quoteMs)}</small></div></div>
  <div class="chart-legend"><div class="segmented">${[1,2,5].map(n=>`<button data-live-interval="${n}" class="${barInterval===n?'active':''}" aria-pressed="${barInterval===n}">${n}m</button>`).join('')}</div><span>${canonical.length} minutos disponibles para la gráfica</span></div>
  ${bars.length?chart({symbol:selectedSymbol,name:universe[selectedSymbol].name,previousClose:bars[0].open,candles:bars,levels:[]},bars.length,barInterval,false,'live',chartMarks()):'<div class="live-empty">Esperando historial o agregados de Massive. No se rellenan intervalos con datos de ejemplo.</div>'}
  <div class="panel-caption"><strong>Historial: ${esc(({loading:'recuperando',ready:'consulta completa',failed:'no recuperado'} as Record<string,string>)[state?.history?.phase??'']??'pendiente')}</strong> · ${esc(state?.history?.detail??'Se carga al conectar.')}<br>Últimas 180 velas visibles; minutos oficiales REST/AM y parciales provisionales por segundo. ${state?.history?.throughMs?`Consulta hasta ${new Date(state.history.throughMs).toLocaleString('es',{timeZone:'America/New_York'})} ET.`:''} Los intervalos sin operaciones elegibles pueden estar vacíos. ▲ E verde bajo la vela: entrada; ▼ S ámbar sobre la vela: salida. E/S: ejecuciones confirmadas de Alpaca Paper; SIM: simulación interna. ${esc(brokerMarksError)} Solo se marcan operaciones guardadas del activo visible dentro de las velas disponibles.</div></section>
  <div data-live-group="flow">
  <section class="panel"><div class="panel-head"><div><span class="eyebrow">OPCIONES ${esc(selectedSymbol)}</span><h2>Cotizaciones y últimas operaciones</h2></div><span class="tag">DATOS DEL PROVEEDOR</span></div><div class="table-wrap"><table><thead><tr><th>CONTRATO</th><th class="numeric">ÚLTIMO</th><th class="numeric">BID</th><th class="numeric">ASK</th><th>EDAD TRADE</th><th>EDAD QUOTE</th></tr></thead><tbody>${options.length?options.map(t=>`<tr><td>${esc(t.symbol)}</td><td class="numeric">${value(t.price)}</td><td class="numeric">${value(t.bid)}</td><td class="numeric">${value(t.ask)}</td><td>${ageMarkup(t.tradeMs)}</td><td>${ageMarkup(t.quoteMs)}</td></tr>`).join(''):'<tr><td colspan="6">Sin eventos de opciones recibidos.</td></tr>'}</tbody></table></div><div class="panel-caption">Base de hasta 6 calls y 6 puts cercanos, más contratos exactos de muros, imanes y objetivos vigentes. Coincidencias estructurales: ${state?.selection?.structuralResolved??0} / ${state?.selection?.structuralRequested??0}; no equivalen a ventanas observadas. Selección: ${esc(state?.selection?.source||'pendiente')} · ${value(state?.selection?.spot)} · vencimiento ${esc(state?.selection?.expiry||'—')}. Revisión de contratos cada minuto; recepción de operaciones y cotizaciones continua. ${state?.selection?.error?esc(state.selection.error):''} El coordinador valida por separado las condiciones de admisión simulada.</div></section>
  <section class="panel"><div class="panel-head"><div><span class="eyebrow">OPCIONES · CONTRATOS SELECCIONADOS</span><h2>Flujo inferido</h2></div><span class="tag">TRADE FRENTE A BID / ASK</span></div>
  <div class="metrics">${(state?.flow?.windows??[1,5,15,30].map(minutes=>({minutes,event_count:0,contracts:0,premium:0,premium_complete:true,bullish_pct:null,bearish_pct:null,direction:'NO DATA'}))).map(w=>`<div class="metric"><span>Ventana ${w.minutes}m</span><div>${w.event_count&&w.premium_complete?Intl.NumberFormat('es',{style:'currency',currency:'USD',notation:'compact'}).format(w.premium):'—'}</div><p>${w.event_count} operaciones · ${w.contracts} contratos</p><p>${esc(classificationNote(w,active&&interpretationFresh(state?.publishedMs)&&state?.options.phase==='streaming'&&state.options.lastEventMs!=null&&state.options.lastEventMs<=Date.now()+1000&&Date.now()-state.options.lastEventMs<=15000))}</p><p>${w.event_count?esc(({ 'BULLISH FLOW':'Sesgo alcista inferido','BEARISH FLOW':'Sesgo bajista inferido','BALANCED FLOW':'Sin predominio','UNCLASSIFIED':'Sin compras/ventas clasificadas','INCOMPLETE':'Primas o cobertura incompletas'} as Record<string,string>)[w.direction]??'Sin datos'):'Sin operaciones observadas'}</p></div>`).join('')}</div>
  <div class="table-wrap"><table><thead><tr><th>HORA ET</th><th>CONTRATO</th><th>LADO INFERIDO</th><th>CONFIANZA</th><th>NBBO</th><th class="numeric">PRIMA USD</th></tr></thead><tbody>${state?.flow?.recent?.length?state.flow.recent.map(e=>`<tr><td>${new Date(e.timestamp_ms).toLocaleTimeString('es',{timeZone:'America/New_York'})}</td><td>${esc(e.ticker)}</td><td>${esc(({BUY:'Compra',SELL:'Venta',UNKNOWN:'Desconocido'} as Record<string,string>)[e.side]??'Desconocido')}</td><td>${esc(({HIGH:'Alta',MEDIUM:'Media',LOW:'Baja'} as Record<string,string>)[e.confidence]??'—')}</td><td>${esc(({valid:'Vigente',missing:'Ausente',stale:'Caducado',future:'Posterior al trade',invalid:'Inválido'} as Record<string,string>)[e.quote_state]??'—')}</td><td class="numeric">${value(e.premium)}</td></tr>`).join(''):'<tr><td colspan="6">Sin operaciones de opciones recibidas para clasificar.</td></tr>'}</tbody></table></div>
  <div class="panel-caption">Inferencia del lado agresor; no identifica apertura/cierre de posiciones ni intención del operador. NBBO válido hasta ${(state?.flow?.quote_max_age_ms??2000)/1000} s antes del trade. Los multiplicadores no verificados dejan la prima sin calcular.<br>Solo eventos observados desde la conexión y de los contratos seleccionados; la selección cambia durante la sesión. No incluye operaciones perdidas durante desconexiones. Descartes por capacidad: ${state?.flow?.capacity_drops??0} · trades rechazados: ${state?.flow?.rejected_trades??0}.</div></section>
  </div><div data-live-group="levels">
  <section class="panel"><div class="panel-head"><div><span class="eyebrow">SNAPSHOT REST · OI DIARIO</span><h2>Interés abierto y referencias</h2></div><button data-refresh-options ${!active||!state?.selection?.spot||state?.optionSnapshotStatus?.phase==='loading'?'disabled':''}>${state?.optionSnapshotStatus?.phase==='loading'?'Consultando…':'Actualizar snapshot'}</button></div>
  <div class="panel-caption">${esc(state?.optionSnapshotStatus?.detail||'Se consulta al conectar. Las operaciones y cotizaciones continúan por WebSocket.')}<br>${state?.optionSnapshotStatus?.observedMs?`Observación ${new Date(state.optionSnapshotStatus.observedMs).toLocaleString('es',{timeZone:'America/New_York'})} ET · ${state.optionSnapshotStatus.contracts} contratos · vencimientos ${esc(state.optionSnapshotStatus.expirations.join(', '))}`:'Sin snapshot recibido.'}<br>OI ausente: ${state?.optionSnapshotStatus?.oiMissing??0} · contratos sin delta/gamma: ${state?.optionSnapshotStatus?.greeksMissing??0}. No se convierten ausentes en cero. ${esc(state?.optionSnapshotStatus?.storageError||'')}</div>
  <div class="table-wrap oi-table"><table><thead><tr><th>CONTRATO</th><th class="numeric">OI OBSERVADO</th><th class="numeric">OI PREVIO</th><th class="numeric">DIFERENCIA</th><th>OBSERVACIÓN PREVIA</th></tr></thead><tbody>${state?.optionSnapshotStatus?.changes?.length?state.optionSnapshotStatus.changes.map(c=>`<tr><td>${esc(c.ticker)}</td><td class="numeric">${c.current??'—'}</td><td class="numeric">${c.previous??'—'}</td><td class="numeric">${c.change??'—'}</td><td>${esc(c.previousObservedDate??'Sin historial comparable')}</td></tr>`).join(''):'<tr><td colspan="5">Sin observaciones de OI para comparar.</td></tr>'}</tbody></table></div>
  <div class="panel-caption">Vista de hasta 60 contratos. Diferencias entre observaciones de fechas distintas; no son entradas/salidas de posiciones en tiempo real. Massive no proporciona una fecha individual de publicación del OI. El historial nuevo se guarda por separado del proyecto Python. Griegas y OI se consultan al conectar, cambiar de vencimiento o fecha ET, y al solicitar una actualización.</div></section>
  </div>
  ${liveGroup('levels',exposurePanel())}
  ${liveGroup('levels',cmePanel(state?.cme,active,state?.publishedMs))}
  ${liveGroup('levels',spxSyntheticPanel(state?.spxSynthetic,active&&state?.options.phase==='streaming',state?.publishedMs))}
  ${liveGroup('levels',structuralReadingPanel(state?.optionSnapshotStatus,active&&interpretationFresh(state?.publishedMs)))}
  ${liveGroup('levels',dexChangePanel(state?.optionSnapshotStatus,active&&interpretationFresh(state?.publishedMs)))}
  ${liveGroup('monitor',scannerPanel(state?.scanner,active,state?.publishedMs))}
  ${liveGroup('flow',localVolumePanel(state?.localVolume,selectedSymbol,active&&state?.options.phase==='streaming',state?.publishedMs))}
  ${liveGroup('flow',optionLiquidityPanel(state?.optionLiquidity,selectedSymbol,active&&state?.options.phase==='streaming',state?.publishedMs))}
  ${liveGroup('flow',flowCoveragePanel(state?.flow?.contract_windows,active&&state?.options?.phase==="streaming",state?.publishedMs,state?.options?.lastEventMs))}
  ${liveGroup('flow',wallFlowPanel(state?.wallFlow,active,state?.publishedMs,state?.options?.lastEventMs))}
  ${liveGroup('levels',adjustedReadingPanel(state?.adjustedReadings,state?.adjustedAlignment,active,state?.publishedMs)+neuroContextLine(state?.neuroContext,selectedSymbol,active))}
  ${liveGroup('levels',wallPullPanel(state?.wallPull,active,state?.publishedMs,state?.stocks?.lastEventMs,state?.options?.lastEventMs))}
  ${liveGroup('levels',structuralPathPanel(state?.structuralPaths,active,state?.publishedMs))}${liveGroup('monitor',decisionReviewPanel(state?.decisionReview,active))}
  ${liveGroup('levels',positionMapPanel(state?.positionMaps,active,state?.publishedMs))}
  ${liveGroup('levels',confirmationPanel(state?.magnetConfirmations,active,state?.publishedMs))}
  ${liveGroup('levels',rangeContextPanel(state?.rangeContext,selectedSymbol,active,state?.publishedMs))}
  ${liveGroup('levels',dailyRangePanel(state?.optionSnapshotStatus,active&&interpretationFresh(state?.publishedMs)))}
  ${liveGroup('levels',structurePanel())}
  ${liveGroup('monitor',analystPanel(analystContext,active,isTauri(),analystBusy,analystError))}
  ${liveGroup('flow',offExchangePanel())}
  <section class="panel" data-live-group="monitor"><div class="panel-head"><div><span class="eyebrow">CALIDAD DE RECEPCIÓN</span><h2>Diagnóstico y registro</h2></div><button data-export-recording ${!state||exporting?'disabled':''}>${exporting?'Guardando…':'Exportar registro local'}</button></div>
  <div class="table-wrap"><table><thead><tr><th>FEED</th><th>P50 / P95 / P99 (MS)</th><th>DUPLICADOS</th><th>FUERA DE ORDEN</th><th>CONFLICTOS</th><th>RELOJ FUTURO</th></tr></thead><tbody>${(['stocks','options'] as const).map(k=>{const q=state?.[k]?.quality;return `<tr><td>${k==='stocks'?'Stocks':'Options'}</td><td>${q?.latencySamples?`${q.p50Ms} / ${q.p95Ms} / ${q.p99Ms}`:'Sin muestras'}</td><td>${q?.duplicates??0}</td><td>${q?.outOfOrder??0}</td><td>${q?.sequenceConflicts??0}</td><td>${q?.futureEvents??0}</td></tr>`;}).join('')}</tbody></table></div>
  <div class="panel-caption">Antigüedad al recibir T/Q respecto al reloj local; no mide exclusivamente la red. Últimas 4.096 muestras por feed.<br>Registro: ${state?.journalRecords??0} / 10.000 eventos · anteriores sobrescritos: ${state?.overwrittenRecords??0}. Identidades retiradas de la deduplicación: ${(state?.stocks?.quality?.identityEvictions??0)+(state?.options?.quality?.identityEvictions??0)}. Trades sin secuencia: ${(state?.stocks?.quality?.unidentifiedTrades??0)+(state?.options?.quality?.unidentifiedTrades??0)}.<br>El registro conserva la ventana recibida, no garantiza una sesión completa ni recupera operaciones perdidas. ${esc(exportMessage)}</div></section>
  </section>`;
}

const visualCadence=new VisualCadence(1000);
let visualHealth='';
let auctionProfileDragging=false;
function refresh(){
 refreshMassiveConnection();
 const root=document.querySelector('#live-workspace');
 if(!root||document.hidden){visualCadence.cancel();return;}
 // Retain every observation; coalesce only the visual work.
 const health=JSON.stringify([root.getAttribute('data-live-view'),active,pending,generation,selectedSymbol,state?.stocks.phase,state?.options.phase,interpretationFresh(state?.publishedMs)]);
 const immediate=health!==visualHealth||renderedSignature==='';
 visualHealth=health;
 visualCadence.run(refreshNow,immediate);
}
function refreshNow() {
  if(document.hidden)return;
  const root = document.querySelector('#live-workspace');
  if (!root||gexDragging||auctionProfileDragging) return;
  if(root.getAttribute('data-live-view')==='top-five') {
    const signature='top-five:'+JSON.stringify([marketNewsRevision(),nativeRevision(),scannerView,state?.scanner,active,interpretationFresh(state?.publishedMs),Math.floor(Date.now()/60000),state?.scanner?.rows.map(r=>r.barEndMs!=null&&r.barEndMs<=Date.now()&&Date.now()-r.barEndMs<=120000)]);
    if(signature===renderedSignature)return;
    renderedSignature=signature;
    updateLiveWorkspace(root,livePage('top-five'));
    const offSelect=document.querySelector<HTMLSelectElement>('[data-off-session]');
  if(offSelect)offSelect.onchange=()=>{offSessionDate=offSelect.value;renderedSignature='';refresh();};
  bindTopFive(refresh);
    return;
  }
  // Expire the selected price even when other symbols keep the feed active.
  const tradeMs=state?.ticks[selectedSymbol]?.tradeMs;
  const priceFreshSignature=String(tradeMs!=null&&tradeMs<=Date.now()+1000&&Date.now()-tradeMs<=15000);
  const signature=JSON.stringify({gapReport:state?.gapReport,dailyAtr:state?.dailyAtr,treasury:state?.treasury,neuroContext:state?.neuroContext,levelDynamics:state?.levelDynamics,levelDynamicsFresh:state?.levelDynamics!=null&&state.levelDynamics.at<=Date.now()&&Date.now()-state.levelDynamics.at<=3000,thetaVega:state?.thetaVega,thetaVegaFresh:state?.thetaVega?.map(r=>r.quoteMs!=null&&Date.now()-r.quoteMs<=60000),brokerMarks,brokerMarksError,assetSlots,watchlistSlot,rangeContext:state?.rangeContext,rangeContextFresh:rangeContextFresh(state?.rangeContext,selectedSymbol,active,state?.publishedMs),localVolume:state?.localVolume,localVolumeFresh:localVolumeFresh(state?.localVolume,selectedSymbol,active&&state?.options.phase==='streaming',state?.publishedMs),analystContext,analystBusy,analystError,analystFresh:analystContext!=null&&Date.now()-Math.min(analystContext.snapshot_generated_at_ms,analystContext.data_quality.evidence_timestamp_ms)<=15000,cme:state?.cme,cmeConversionsFresh:state?.cme?.conversions.map(r=>Date.now()-r.oldestPriceMs<=15000),cmeFresh:state?.cme?.nqTimestampMs!=null&&Date.now()-state.cme.nqTimestampMs<=15000,spxReading:state?.spxReading,spxReadingFresh:spxReadingFresh(state?.spxReading,state?.spxSynthetic,active&&state?.options.phase==='streaming',state?.publishedMs),spxSynthetic:state?.spxSynthetic,spxFresh:spxUsable(state?.spxSynthetic,active&&state?.options.phase==='streaming',state?.publishedMs),decisionReview:state?.decisionReview,structuralPaths:state?.structuralPaths,optionLiquidity:state?.optionLiquidity,scanner:state?.scanner,adjustedReadings:state?.adjustedReadings,adjustedAlignment:state?.adjustedAlignment,wallPull:state?.wallPull,wallFlow:state?.wallFlow,positionMaps:state?.positionMaps,rangeFresh:rangeFresh(state?.optionSnapshotStatus),magnetConfirmations:state?.magnetConfirmations,interpretationFresh:interpretationFresh(state?.publishedMs),dexFresh:dexFreshnessSignature(state?.optionSnapshotStatus),active,pending,error,exportMessage,exporting,journalRecords:state?.journalRecords,overwrittenRecords:state?.overwrittenRecords,selectedSymbol,gexHorizon,gexHeight,gexPriceRange,gexDate,gexBefore,gexPanOffset,gexVisible,gexExpanded,chartHistory:state?.chartHistory,barInterval,stocks:state?.stocks,options:state?.options,ticks:state?.ticks,bars:state?.bars,minutes:state?.minutes,history:state?.history,metrics:state?.metrics,optionSnapshotStatus:state?.optionSnapshotStatus,flow:state?.flow,offExchange:state?.offExchange,selection:state?.selection,fresh:[state?.stocks,state?.options].map(f=>f?.lastEventMs!=null&&Date.now()-f.lastEventMs<=15000)});
  if(signature+priceFreshSignature===renderedSignature){
    root.querySelectorAll<HTMLElement>('[data-event-age]').forEach(e=>{const label=age(e.dataset.eventAge?Number(e.dataset.eventAge):null);if(e.textContent!==label)e.textContent=label;});
    return;
  }
  renderedSignature=signature+priceFreshSignature;
  const element=document.activeElement;
  const focusSelector=element?.hasAttribute('data-live-action')?'[data-live-action]':element?.hasAttribute('data-live-interval')?`[data-live-interval="${element.getAttribute('data-live-interval')}"]`:null;
  const expanded = new Set([...root.querySelectorAll<HTMLDetailsElement>("[data-horizon-details][open]")].map(e=>e.dataset.horizonDetails));
  const selectedView=root.getAttribute('data-live-view');
  updateLiveWorkspace(root,livePage(selectedView==='gamma-live'||selectedView==='spx'||selectedView==='top-five'||selectedView==='gex'||selectedView==='flow'||selectedView==='maps'||selectedView==='levels'||selectedView==='quick'||selectedView==='detailed'||selectedView==='asset'?selectedView:'monitor'));
  document.querySelectorAll<HTMLDetailsElement>("[data-horizon-details]").forEach(e=>{e.open=expanded.has(e.dataset.horizonDetails);});
  bindLive();
  if (focusSelector) document.querySelector<HTMLElement>(focusSelector)?.focus({preventScroll:true});
}

export function bindLive() {
  bindTwoMaps(()=>{renderedSignature='';refreshNow();});
  bindLevelCardSpacing();
  bindAuctionProfileNavigation(()=>{renderedSignature='';refreshNow();},value=>{auctionProfileDragging=value;});
  bindParallelRoute(()=>{renderedSignature='';refreshNow();});
  bindRouteJournal();
  try{bindGammaLive(state,selectedSymbol,active,()=>{renderedSignature='';refreshNow();});}catch(e){const status=document.getElementById('route-chart-stats');if(status)status.textContent='Gráfica: '+String(e).slice(0,240);}
  void loadBrokerMarks();
  const offSelect=document.querySelector<HTMLSelectElement>('[data-off-session]');
  if(offSelect)offSelect.onchange=()=>{offSessionDate=offSelect.value;renderedSignature='';refresh();};
  bindTopFive(refresh);
  bindWatchlist();
  const navigateGex=(direction:number)=>{
   gexPanOffset=0;
   const rows=gexCandles.filter(c=>c.sessionDate===(gexDate||gexCandles.at(-1)?.sessionDate));
   if(!rows.length)return;
   let index=gexBefore==null?rows.length-1:rows.reduce((last,c,i)=>c.timestampMs<=gexBefore!?i:last,-1);
   index=Math.max(Math.min(rows.length-1,gexVisible-1),Math.min(rows.length-1,index+direction*Math.max(1,Math.floor(gexVisible/3))));
   gexBefore=index===rows.length-1?undefined:rows[index].timestampMs;
  };
  const dateSelector=document.querySelector<HTMLSelectElement>('[data-gex-date]');
  if(dateSelector)dateSelector.onchange=()=>{gexDate=dateSelector.value;gexPriceRange=undefined;gexBefore=undefined;gexPanOffset=0;refresh();};
  document.querySelectorAll<HTMLButtonElement>('[data-gex-action]').forEach(b=>b.onclick=()=>{
   switch(b.dataset.gexAction){case 'older':navigateGex(-1);break;case 'newer':navigateGex(1);break;case 'zoom-in':gexVisible=Math.max(8,Math.floor(gexVisible*.75));break;case 'zoom-out':gexVisible=Math.min(1000,Math.ceil(gexVisible/ .75));break;case 'latest':gexPriceRange=undefined;gexDate='';gexBefore=undefined;gexPanOffset=0;gexVisible=48;break;case 'expand':gexExpanded=!gexExpanded;break;}refresh();
  });
  bindDrawingTools(selectedSymbol,{
   refresh:()=>{renderedSignature='';refresh();},busy:v=>{gexDragging=v;},
   preview:()=>livePage('gex'),
   height:n=>{gexHeight=n;},scale:range=>{gexPriceRange=range;},
   zoom:factor=>{gexVisible=Math.max(8,Math.min(1000,Math.round(gexVisible*factor)));},
   pan:(offset,shift)=>{
    // Anchor a manually moved window so incoming candles cannot shift it.
    if(gexBefore==null)gexBefore=gexCandles.filter(c=>c.sessionDate===(gexDate||gexCandles.at(-1)?.sessionDate)).at(-1)?.timestampMs;
    gexPanOffset+=offset;
    if(shift){const svg=document.querySelector<SVGSVGElement>('[data-gex-svg]');if(svg){const lo=Number(svg.dataset.priceMin),hi=Number(svg.dataset.priceMax);gexPriceRange=[Math.max(.0001,lo+shift),Math.max(.0002,hi+shift)];}}
   }
  });

  const gexSelector=document.querySelector<HTMLSelectElement>('[data-gex-horizon]');
  if(gexSelector)gexSelector.onchange=()=>{gexHorizon=gexSelector.value;renderedSignature='';refresh();};
  bindAnalystTargets(()=>{renderedSignature='';refresh();});
  document.querySelectorAll<HTMLButtonElement>('[data-block-period]').forEach(b=>b.onclick=()=>{selectBlockPeriod(b.dataset.blockSymbol!,b.dataset.blockPeriod!);renderedSignature='';refresh();});
  document.querySelectorAll<HTMLButtonElement>('[data-live-asset]').forEach(b=>b.onclick=()=>{void selectLiveAsset(b.dataset.liveAsset!);});
  document.querySelectorAll<HTMLInputElement>('[data-slot-input]').forEach(input=>input.oninput=()=>{const slot=Number(input.dataset.slotInput);assetDrafts[slot]=input.value;showWatchlist(slot,input.value);});
  document.querySelectorAll<HTMLFormElement>('[data-asset-slot]').forEach(form=>form.onsubmit=e=>{e.preventDefault();void selectLiveAsset(new FormData(form).get('symbol')?.toString()??'',Number(form.dataset.assetSlot));});
  const analystButton=document.querySelector<HTMLButtonElement>('[data-analyst-context]');
  if(analystButton)analystButton.onclick=async()=>{
    const token=generation;analystBusy=true;analystError='';refresh();
    try{const result=await invoke<AnalystContext>('get_analyst_context');if(token===generation&&active&&result.symbol===selectedSymbol)analystContext=result;}
    catch(e){if(token===generation)analystError=String(e);}
    finally{analystBusy=false;refresh();}
  };
  const snapshotButton=document.querySelector<HTMLButtonElement>('[data-refresh-options]');
  if(snapshotButton)snapshotButton.onclick=async()=>{try{await invoke('refresh_option_snapshot');}catch(e){error=String(e);refresh();}};

  const exportButton=document.querySelector<HTMLButtonElement>('[data-export-recording]');
  if(exportButton)exportButton.onclick=async()=>{exporting=true;exportMessage='';refresh();try{const path=await invoke<string>('export_live_recording');exportMessage=`Guardado en ${path}`;}catch(e){exportMessage=String(e);}finally{exporting=false;refresh();}};

  const legacyToggle=document.querySelector<HTMLInputElement>('[data-resume-legacy]');
  if(legacyToggle)legacyToggle.onchange=()=>{if(!active&&!pending){resumeLegacy=legacyToggle.checked;refresh();}};
  const paperToggle=document.querySelector<HTMLInputElement>('[data-paper-entries]');
  if(paperToggle)paperToggle.onchange=()=>{if(!active&&!pending&&selectedSymbol==='QQQ'){paperEntries=paperToggle.checked;refresh();}};
  document.querySelectorAll<HTMLButtonElement>('[data-live-interval]').forEach(button=>button.onclick=()=>{barInterval=Number(button.dataset.liveInterval);refresh();});
  bindMassiveConnection();
}
function bindMassiveConnection() {
  document.querySelector<HTMLButtonElement>('[data-switch-connection]')?.addEventListener('click',event=>{
   const next=(event.currentTarget as HTMLButtonElement).dataset.switchConnection;
   if(next==='direct'||next==='receiver')void switchDataConnection(next);
  });
  const button = document.querySelector<HTMLButtonElement>('[data-live-action]');
  if (button) button.onclick = async () => {
    if(pending||!isTauri())return;
    pending = true; error = ''; refresh();
    try {
      if (active) {
        generation++; analystContext=undefined;analystError='';await invoke('stop_live'); active = false; paperEntries = false; resumeLegacy = false; channel = undefined;
      } else {
        if(document.querySelector<HTMLElement>('#live-workspace')?.dataset.liveView==='spx'){paperEntries=false;resumeLegacy=false;}
        const token = ++generation;
        state = undefined;
        channel = new Channel<LiveState>();
        channel.onmessage = payload => { if (token === generation && payload.symbol===selectedSymbol) { state = payload; cvdChartEngine.observe(payload,selectedSymbol,active,Date.now()); blockMemory.remember(payload); updateEtfCvd(payload.etfCvd,payload.stocks.phase==="streaming",payload); consumeBackgroundAlerts(payload.priceAlerts??[]); readRouteLive(payload,selectedSymbol,active); collectGammaLearning(payload); refresh(); } };
        await invoke('start_live',{connectionMode,priceAlerts:allPriceAlerts(),symbol:selectedSymbol,entriesEnabled:paperEntries,resumeLegacy,onMessage:channel}); active = true;
      }
    } catch (e) { error = String(e); }
    finally { pending = false; refresh(); }
  };
}
async function switchDataConnection(mode:'receiver'|'direct'):Promise<void>{
 if(pending||!isTauri())return;
 pending=true;error='';refresh();
 const reconnect=active;const previous=connectionMode;
 try{
  const attach=async()=>{
   connectionMode=mode;
   if(!reconnect)return;
   state=undefined;const token=++generation;channel=new Channel<LiveState>();
   channel.onmessage=payload=>{if(token===generation&&payload.symbol===selectedSymbol){state=payload;cvdChartEngine.observe(payload,selectedSymbol,active,Date.now());blockMemory.remember(payload);updateEtfCvd(payload.etfCvd,payload.stocks.phase==='streaming',payload);consumeBackgroundAlerts(payload.priceAlerts??[]);readRouteLive(payload,selectedSymbol,active);collectGammaLearning(payload);refresh();}};
   await invoke('start_live',{connectionMode:mode,priceAlerts:allPriceAlerts(),symbol:selectedSymbol,entriesEnabled:false,resumeLegacy:false,onMessage:channel});active=true;
  };
  await transferConnection(async()=>{if(reconnect){generation++;await invoke('stop_live');active=false;channel=undefined;state=undefined;paperEntries=false;resumeLegacy=false;analystContext=undefined;analystError='';}},attach);
 }catch(e){connectionMode=previous;error=String(e);}
 finally{pending=false;renderedSignature='';refresh();}
}
// This only updates age labels; market data arrives over IPC, never by polling REST.
setInterval(()=>{ continuousAudit.observe(state,selectedSymbol,active); momentumShadow.observe(state,selectedSymbol,active); if (state && !document.hidden) refresh(); void loadBrokerMarks(); },1000);
document.addEventListener('visibilitychange',()=>{if(!document.hidden){renderedSignature='';refresh();}});
