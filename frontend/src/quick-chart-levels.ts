import {chartContractReport} from './contract-profile';
import type {LiveState} from './live';
import {gammaRouteChart} from './gamma-live';
import {currentMapHorizon} from './reading-display';
import {readPriceNow} from './price-now';
import {analystTargetsPanel} from './analyst-targets';
import {dayMap} from './day-map';
import {interpretationFresh} from './confirmations';
import {flowFour} from './flow-four';
import {controlOutlook} from './control-outlook';
import type {ControlReading} from './intraday-control';
import './quick-chart-levels.css';

export function quickChartLevels(s:LiveState|undefined,symbol:string,active:boolean,now=Date.now(),reading?:ControlReading):string {
 const h=active&&s?.symbol===symbol?currentMapHorizon(s.optionSnapshotStatus,now):undefined;
 const quote=readPriceNow(s,symbol,active,now);
 // Reuse the existing cards, including wall pulls, strengths and reading details.
 // The header and volume companions are omitted only in this narrow column.
 const flowFresh=active&&s?.symbol===symbol&&s.options?.phase==='streaming'&&interpretationFresh(s.publishedMs,now)&&s.options.lastEventMs!=null&&s.options.lastEventMs<=now&&now-s.options.lastEventMs<=15000&&s.flow?.asof_ms!=null&&s.flow.asof_ms<=now&&now-s.flow.asof_ms<=3000;
 const map=dayMap(h,!!h,s?.flow?.contract_windows??[],flowFresh,quote.ready?quote.spot:null,'',reading?controlOutlook(reading):undefined,true,'',undefined,undefined,'',{hideLiquidity:true,barrierRows:s?.flow?.bid_ask,oiReport:chartContractReport(s,symbol,active,h,now)});
 const template=map.replace(/<header class="reading-toolbar[\s\S]*?<\/header>/,'');
 return `<div class="quick-chart-levels">${gammaRouteChart(s,symbol,active,now,true)}<aside class="quick-level-boxes levels-route-design" aria-label="Niveles y objetivos de analistas">${template}${flowFour(s,symbol,active,now,undefined,false)}${analystTargetsPanel(symbol,quote.ready?quote.spot??null:null)}</aside></div>`;
}


