import {it,expect} from 'vitest';
import {chart,sparkline} from './chart';
import {paperChartMarks,brokerChartMarks} from './paper-chart';
import type {Asset} from './model';
const asset:Asset={symbol:'QQQ',name:'QQQ',previousClose:100,levels:[],candles:[0,120000].map(timestampMs=>({timestampMs,open:100,high:102,low:99,close:101,volume:10}))};
it('only anchors markers inside existing candles and escapes labels',()=>{
 const html=chart(asset,2,1,false,'live',[{timestampMs:1,price:100,label:'<entry>',kind:'entry'},{timestampMs:60001,price:100,label:'gap',kind:'exit'},{timestampMs:180000,price:100,label:'future',kind:'exit'}]);
 expect(html).toContain('&lt;entry&gt;');expect(html).not.toContain('gap');expect(html).not.toContain('future');expect(html.match(/class="paper-chart-mark"/g)?.length).toBe(1);
 expect(chart(asset,0,1,false)).not.toContain('NaN');expect(sparkline(asset,1)).not.toContain('NaN');
});
it('marks use the actual held symbol, never the selected review symbol',()=>{
 const review={reviewedMs:1,symbol:'SPY',positionSymbol:'QQQ',reason:'WAIT',entriesEnabled:false,position:{state:'POSITION_CLOSED',tradeId:7,trade:{side:'CALL',entry:100,current:102,stop:99,pnlPoints:2,exitReason:null,openedMs:1,closedMs:120001}}};
 expect(paperChartMarks(review,'SPY')).toEqual([]);expect(paperChartMarks(review,'QQQ').map(m=>m.kind)).toEqual(['entry','exit']);
 const saved={...review,positionHistory:[{id:7,symbol:'QQQ',trade:{entry:100,current:102,openedMs:1,closedMs:120001}}]};
 expect(paperChartMarks(saved,'QQQ')).toHaveLength(2);
 expect(paperChartMarks({...saved,position:undefined},'QQQ')).toHaveLength(2);
});

it('permits fractional horizontal positioning and empty future space without invented candles',()=>{
 const html=chart(asset,2,1,false,'live',[],{prepared:true,gex:true,viewport:{start:.25,count:4}});
 expect(html).toContain('data-view-start="0.25"');expect(html).toContain('data-step="186.5"');
 expect(html.match(/class="candle-volume"/g)).toHaveLength(2);
 expect(html).not.toContain('NaN');expect(html).toContain('clip-path="url(#candle-bounds)"');
});

const executed={symbol:'QQQ260928C00745000',side:'CALL',phase:'done',reason:'filled',buy_id:'b1',entry_time:'2026-09-25T14:00:12Z',exit_time:'2026-09-25T14:02:40Z',entry_premium:2,exit_premium:2.4};
it('uses broker fill times, filters the underlying and deduplicates current/history',()=>{
 const marks=brokerChartMarks([executed],executed,'QQQ');
 expect(marks).toHaveLength(2);expect(marks[0].timestampMs).toBe(Date.parse(executed.entry_time));
 expect(marks[0].price).toBeUndefined();expect(marks[0].source).toBe('broker');
 expect(brokerChartMarks([executed],undefined,'SPY')).toEqual([]);
 expect(brokerChartMarks([{...executed,symbol:'O:'+executed.symbol}],undefined,'QQQ')).toHaveLength(2);
});
it('does not mark pending entries or unfilled exits or future executions',()=>{
 expect(brokerChartMarks([{...executed,phase:'buy_pending',entry_time:null,entry_premium:null}],undefined,'QQQ')).toEqual([]);
 expect(brokerChartMarks([{...executed,phase:'sell_pending',exit_time:null,exit_premium:null}],undefined,'QQQ').map(m=>m.kind)).toEqual(['entry']);
 expect(brokerChartMarks([executed],undefined,'QQQ',Date.parse(executed.entry_time)-1)).toEqual([]);
 expect(brokerChartMarks([{...executed,exit_time:'2026-09-25T13:00:00Z'}],undefined,'QQQ')).toHaveLength(1);
});
it('draws entry below the wick and exit above it, without using option premium as stock price',()=>{
 const start=Date.parse('2026-09-25T14:00:00Z');
 const a={...asset,candles:[{timestampMs:start,open:744,high:746,low:743,close:745,volume:10}]};
 const marks=brokerChartMarks([{...executed,exit_time:'2026-09-25T14:00:45Z'}],undefined,'QQQ');
 let geom:any;
 const html=chart(a,1,1,false,'live',marks,{prepared:true,gex:true,overlay:g=>{geom=g;return '';}});
 const points=[...html.matchAll(/<polygon points="[^,]+,([\d.\-]+)/g)].map(m=>Number(m[1]));
 expect(points).toHaveLength(2);expect(points[0]).toBeGreaterThan(geom.y(743));expect(points[1]).toBeLessThan(geom.y(746));
 expect(geom.min).toBeGreaterThan(700);expect(html).toContain('data-mark-source="broker"');
});
