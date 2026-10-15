import {structuralMapPreview} from './structural-map-preview';
import {chartContractReport} from './contract-profile';
import {gapReading} from './gap-engine';
import {currentComponentRead,componentLabel,componentQuickActors,componentQuickPercent} from './qqq-components';
import {vannaEngine} from './vanna-engine';
import {routeProbabilityReading} from './route-probability-reading';
import {baselineReading} from './arrival-baseline-reading';
import {routeChartReading} from './route-live';
import {gammaRouteChart} from './gamma-live';
import {quickTopStrikes} from './quick-top-strikes';
import {dealerQuickLink} from './dealer-quick-link';
import {optionsCvdPanel} from './options-cvd';
import type {LiveState} from './live';
import type {ControlReading} from './intraday-control';
import {auctionMapPanel,quickMapStack} from './auction-map-panel';
import {auctionHistoryReference} from './auction-history';
import {controlOutlook} from './control-outlook';
import {readDirectionScore,directionBadge} from './direction-scanner';
import {directionLevels} from './direction-levels';
import {intradayAtr} from './intraday-atr';
import {atrContext} from './atr-context';
import {dayMap} from './day-map';
import {currentMapHorizon} from './reading-display';
import {routeVolatility} from './route-design';
import {flowFour} from './flow-four';
import {blockPressurePanel} from './block-pressure';

import {routePreview} from './route-preview';
import {selectedPrice,selectedPriceMarkup} from './selected-price';
import {rangeFresh} from './daily-range';
import {interpretationFresh} from './confirmations';
/** One timestamp and one shared ATR calculation for the whole quick reading. */
export function quickReadingPanels(s:LiveState|undefined,symbol:string,active:boolean,input:ControlReading,now=Date.now()):string {
 const atr=intradayAtr(s,symbol,active,now);atr.context=atrContext(s,symbol,active,atr,now);
 const reading:ControlReading={...input,atr},direction=readDirectionScore(s,reading,active,now),linked=directionLevels(s,direction,symbol,active,now);
 reading.vannaEngine=vannaEngine(s,reading,active,now);linked.vanna=reading.vannaEngine.levels;
 if(reading.strikeReading)reading.strikeReading={rows:reading.strikeReading.rows.map(row=>({...row,vanna:reading.vannaEngine!.levels.find(l=>l.name===row.name&&l.strike===row.strike&&l.expiration===row.expiration)?.support}))};
 reading.gapReading=gapReading(s,symbol,active,reading.marketEvidence,atr,now);
 reading.readingAt=now;reading.dealerLink=dealerQuickLink(s,reading,direction,linked,active,now);
 const snapshot=s?.optionSnapshotStatus,h=currentMapHorizon(snapshot,now),fresh=active&&s?.symbol===symbol&&!!h;
 const quote=selectedPrice(symbol,s,active,now);
 const flowFresh=active&&s?.symbol===symbol&&s.options.phase==='streaming'&&interpretationFresh(s.publishedMs,now)&&s.options.lastEventMs!=null&&s.options.lastEventMs<=now&&now-s.options.lastEventMs<=15000;
 const route=routePreview(s,reading,active,now,atr);
 const mapStack=quickMapStack(structuralMapPreview(symbol,s,active,now,atr,reading.vannaEngine),auctionMapPanel(symbol,active,now,s?.auctionMap??undefined,quote.live?quote.value:null,auctionHistoryReference(symbol,s?.symbol===symbol?s.chartHistory:undefined,now)));
 const routeProbability=routeChartReading(symbol,now)?.probability??routeProbabilityReading(symbol,now);
 const probability=routeProbability.call!=null||routeProbability.put!=null?routeProbability:active&&s?.symbol===symbol?baselineReading(symbol,now)??routeProbability:routeProbability;
 return `<div class="quick-command-layout route-command-layout"><section class="levels-route-design${symbol==='QQQ'?' qqq-components-layout':''}">${dayMap(h,fresh,s?.flow?.contract_windows??[],flowFresh,quote.live?quote.value:null,selectedPriceMarkup(quote,true),controlOutlook(reading),true,directionBadge(direction,atr,probability,symbol==='QQQ'?{label:componentLabel(currentComponentRead(now)),detail:currentComponentRead(now).reason,actors:componentQuickActors(currentComponentRead(now)),labelMarkup:componentQuickPercent(currentComponentRead(now))}:undefined),linked,atr,optionsCvdPanel(s,symbol,active,now,atr,reading.marketEvidence),{barrierRows:s?.flow?.bid_ask,oiReport:chartContractReport(s,symbol,active,h,now)})}<div class="flow-column">${routeVolatility(reading,now,atr)}${flowFour(s,symbol,active,now,atr,false)}${blockPressurePanel(s,symbol,active,now,atr)}</div></section>${mapStack}</div>${gammaRouteChart(s,symbol,active,now,true)}${route}${quickTopStrikes(s,symbol,active,now)}`;
}
