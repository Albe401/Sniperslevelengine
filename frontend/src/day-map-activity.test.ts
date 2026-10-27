import {describe,it,expect} from 'vitest';
import {dayMap,mapLevelReading,type MapOutlook} from './day-map';
import {readLevelActivities,type LevelActivityInput} from './level-activity';
import {activityStyle} from './level-activity-view';
import {strikeKey} from './strike-key';
import type {Horizon} from './live';
import type {CvdRead} from './cvd';
import type {AuctionReading} from './auction-map-panel';
import type {BidAskWindow} from './strike-bid-ask';

// Same explicitly synthetic source/unit separation as level-activity-preview.ts;
// this verifies presentation contracts, not positive market acceptance.
const at=Date.parse('2026-10-26T19:30:30Z'),date='2026-10-26';
function fixture(){
 const zone=(price:number,buy:number,sell:number,unknown:number)=>({lower:price,upper:price+.01,firstMs:at-59000,lastMs:at-100,shares:buy+sell+unknown,buy,sell,unknown});
 const period={buy:3500,sell:21000,unknown:3250,delta:-17500,coveragePct:88.288,sinceMs:at-3600000,lastMs:at-100,changeOne:-17500,changeFive:-17500,gaps:0};
 const cvd={symbol:'QQQ',date,at,available:true,regular:period,premarket:{...period},rejected:0,executionEvidence:{version:'execution-evidence-v1',status:'observing',observedSinceMs:at-3600000,windows:[{seconds:60,startMs:at-60000,endMs:at,complete:true,classifiedPct:88,excludedZones:0,zones:[zone(710,2000,8000,1000),zone(715,1000,12000,2000),zone(720,500,1000,250)]}],absorption:{version:'execution-absorption-v1',status:'pending',reason:'Synthetic visual example, not market data',candidates:[]}}} as unknown as CvdRead;
 const auction={version:'auction-map-ticks-v1',symbol:'QQQ',asOfMs:at,provisional:true,acceptedTrades:600,totalVolume:42000,pocLow:709.9,pocHigh:710.1,val:709,vah:721,bins:[{low:709.9,high:710.1,volume:21000,brackets:12,volumeRatio:1,timeRatio:1,kind:'acceptance'},{low:714.9,high:715.1,volume:12600,brackets:9,volumeRatio:.6,timeRatio:.75,kind:'acceptance'},{low:719.9,high:720.1,volume:8400,brackets:6,volumeRatio:.4,timeRatio:.5,kind:'neutral'}],reaction:'pending',routes:[],calibrated:false} satisfies AuctionReading;
 const optionRows:BidAskWindow[]=[];
 for(const [strike,bidCall,bidPut] of [[710,200,250],[715,500,600],[720,10000,2000]])for(const kind of ['call','put'])optionRows.push({ticker:`EXAMPLE-${strike}-${kind}`,kind,strike,expiration:date,covered:true,bid:kind==='call'?bidCall:bidPut,ask:200,unknown:50});
 const horizon={mode:'DAY',label:'Día',expirations:[date],observedMs:at,referenceSource:'Synthetic visual example',structure:{complete:true,liquidityComplete:true,selected:{M1:715},mainMagnetScore:68,callWall:{strike:720,score:65},putWall:{strike:710,score:61},routes:[]}} as unknown as Horizon;
 return {cvd,auction,optionRows,horizon};
}
type Fixture=ReturnType<typeof fixture>;
function render(f:Fixture,options:{now?:number;flowFresh?:boolean;withoutActivity?:boolean;activity?:Partial<LevelActivityInput>}={}){
 const activity={symbol:'QQQ',active:true,publishedMs:at,cvd:f.cvd,auction:f.auction,optionAtMs:at,...options.activity};
 return dayMap(f.horizon,true,[],options.flowFresh??true,710.04,'<strong>QQQ $710.04</strong>',undefined,true,'',undefined,undefined,'',{hideLiquidity:true,barrierRows:f.optionRows,activity:options.withoutActivity?undefined:activity},options.now??at);
}
function cards(html:string){return [...html.matchAll(/<article\b([^>]*)>([\s\S]*?)<\/article>/g)].map(match=>({outer:match[1],html:match[2]}));}
function card(html:string,label:string){const result=cards(html).find(item=>item.html.includes(`<span>${label}</span>`));expect(result,`missing ${label}`).toBeDefined();return result!;}
function chipClasses(html:string,source:'acceptance'|'stock'|'option'){
 const match=html.match(new RegExp(`class="(level-activity-chip level-activity-${source}[^\"]*)"`));
 expect(match,`missing ${source} chip`).toBeTruthy();return match![1];
}
function innerStyle(html:string){const tag=html.match(/<div class="panel day-map-level"([^>]*)>/);expect(tag).toBeTruthy();return tag![1];}
const meterValues=(html:string)=>[...html.matchAll(/class="level-strength-track"[^>]*aria-valuenow="([^"]+)"/g)].map(match=>match[1]);

describe('day map activity integration',()=>{
 it('keeps the activity strip and source legend behind the existing closed reading toggle',()=>{
  const html=render(fixture());
  expect(html).not.toContain('level-activity-legend');
  for(const label of ['PUT WALL','IMÁN PRINCIPAL','CALL WALL']){
   const body=card(html,label).html,details=body.match(/<details class="map-level-reading"[^>]*>/g)??[];
   expect(details).toHaveLength(1);expect(details[0]).not.toMatch(/\bopen(?:\s|=|>)/);
   const beforeDetails=body.slice(0,body.indexOf('<details class="map-level-reading"'));
   expect(beforeDetails).not.toContain('level-activity-chips');expect(beforeDetails).not.toContain('level-activity-evidence');
   const detailBody=body.match(/<div class="map-reading-detail">([\s\S]*?)<\/details>/)?.[1]??'';
   expect(detailBody).toContain('level-activity-chips');expect(detailBody).toContain('Iluminación del cuadro');
   expect(detailBody).toContain('Ventas de acciones · 60 s · acciones');expect(detailBody).toContain('Ventas de opciones · 5 min · contratos');
   expect(detailBody).toContain('Aceptación AMT · perfil del subyacente');
  }
 });
 it('keeps every activity color identical while changing only where evidence is displayed',()=>{
  const f=fixture(),html=render(f),prices=[720,715,710];
  const observed=readLevelActivities(prices,{symbol:'QQQ',active:true,publishedMs:at,cvd:f.cvd,auction:f.auction,optionRows:f.optionRows,expirations:[date],optionFresh:true,optionAtMs:at},at);
  for(const [label,price] of [['CALL WALL',720],['IMÁN PRINCIPAL',715],['PUT WALL',710]] as const){
   expect(innerStyle(card(html,label).html).trim()).toBe(activityStyle(observed.get(strikeKey(price)!)));
  }
 });
 it('preserves the same summary and escaped evidence with or without an outlook',()=>{
  const f=fixture(),observed=readLevelActivities([710],{symbol:'QQQ',active:true,publishedMs:at,cvd:f.cvd,auction:f.auction,optionRows:f.optionRows,expirations:[date],optionFresh:true,optionAtMs:at},at).get(strikeKey(710)!)!;
  observed.stockSales.detail='<script>unsafe</script> & quantities';
  const outlook={rows:[],level:'Nivel',levelDetail:'',change:'Cambio',changeDetail:'',route:'Ruta',routeDetail:'',invalidate:'Condición',invalidateDetail:''} as unknown as MapOutlook;
  for(const context of [undefined,outlook]){
   const html=mapLevelReading(['PUT WALL'],710,context,'Composición','Lectura +','Evidencia existente',observed);
   expect(html).toContain('<summary><span>Composición</span><b>Lectura +</b></summary>');
   expect(html).toContain('<div class="map-reading-detail"><div class="level-activity-evidence">');
   expect(html).toContain('Evidencia existente');expect(html).toContain('&lt;script&gt;unsafe&lt;/script&gt; &amp; quantities');
   expect(html).not.toContain('<script>');
  }
 });
 it('places independent activity styles inside cards while preserving one outer grid style',()=>{
  const html=render(fixture()),visible=cards(html);
  expect(visible.filter(item=>item.outer.includes('level-activity-card'))).toHaveLength(3);
  for(const item of visible){
   expect(item.outer.match(/\bstyle=/g)).toHaveLength(1);
   expect(item.outer).toMatch(/style="grid-row:\d+"/);
   expect(item.outer).not.toContain('--activity-');
  }
  for(const label of ['PUT WALL','IMÁN PRINCIPAL','CALL WALL']){
   const inner=innerStyle(card(html,label).html);
   expect(inner.match(/\bstyle=/g)).toHaveLength(1);
   expect(inner).toContain('--activity-stock-fill:');expect(inner).toContain('--activity-option-fill:');expect(inner).toContain('--activity-acceptance-fill:');
   expect(inner).not.toContain('grid-row');expect(inner).not.toContain('--node-color');
  }
 });
 it('keeps the model scores, meters, levels and source inputs unchanged',()=>{
  const f=fixture(),before=JSON.stringify(f),plain=render(f,{withoutActivity:true}),colored=render(f);
  expect(meterValues(colored)).toEqual(meterValues(plain));expect(meterValues(colored)).toEqual(['65','68','61']);
  for(const score of ['65.0/100','68.0/100','61.0/100'])expect(colored).toContain(score);
  for(const label of ['PUT WALL','IMÁN PRINCIPAL','CALL WALL'])expect(card(colored,label).html.match(/<strong>([^<]+)<\/strong>/)?.[1]).toBe(card(plain,label).html.match(/<strong>([^<]+)<\/strong>/)?.[1]);
  expect(plain).not.toContain('level-activity-chip');expect(JSON.stringify(f)).toBe(before);
 });
 it('selects different mapped winners for acceptance, shares and option contracts',()=>{
  const html=render(fixture()),put=card(html,'PUT WALL').html,magnet=card(html,'IMÁN PRINCIPAL').html,call=card(html,'CALL WALL').html;
  expect(chipClasses(put,'acceptance')).toContain('is-max');expect(chipClasses(put,'stock')).not.toContain('is-max');expect(chipClasses(put,'option')).not.toContain('is-max');
  expect(chipClasses(magnet,'stock')).toContain('is-max');expect(chipClasses(magnet,'acceptance')).not.toContain('is-max');expect(chipClasses(magnet,'option')).not.toContain('is-max');
  expect(chipClasses(call,'option')).toContain('is-max');expect(chipClasses(call,'stock')).not.toContain('is-max');expect(chipClasses(call,'acceptance')).toContain('is-pending');
  expect(put).toContain('>Alta</b>');expect(magnet).toContain('>12K</b>');expect(call).toContain('>12K</b>');
  expect(magnet).toContain('unidad: acciones');expect(call).toContain('unidad: contratos');expect(html).toContain('últimos 60 s');expect(html).toContain('últimos 5 min');
 });
 it('makes expired activity neutral while retaining the original structural score',()=>{
  const html=render(fixture(),{now:at+20000,flowFresh:false});
  for(const label of ['PUT WALL','IMÁN PRINCIPAL','CALL WALL']){
   const item=card(html,label).html;
   expect(item.match(/level-activity-chip [^\"]*is-pending/g)).toHaveLength(3);expect(item).not.toContain('is-max');expect(item).not.toContain('★');
   expect(innerStyle(item)).toMatch(/(?:--activity-(?:acceptance|stock|option)-(?:fill|edge):0\.0000;?){6}/);
  }
  expect(meterValues(html)).toEqual(['65','68','61']);
 });
 it('labels an explicit capture and preserves separate observed maxima',()=>{
  const html=render(fixture(),{activity:{active:false,captured:true}});
  expect(html.match(/level-activity-chips is-captured/g)).toHaveLength(3);
  expect(html).toContain('Observación guardada en la captura; no lectura en vivo.');
  expect(chipClasses(card(html,'PUT WALL').html,'acceptance')).toContain('is-max');
  expect(chipClasses(card(html,'IMÁN PRINCIPAL').html,'stock')).toContain('is-max');
  expect(chipClasses(card(html,'CALL WALL').html,'option')).toContain('is-max');
 });
 it('keeps underlying observations independent when DAY options are unavailable',()=>{
  const html=render(fixture(),{flowFresh:false});
  expect(chipClasses(card(html,'CALL WALL').html,'option')).toContain('is-pending');
  expect(chipClasses(card(html,'IMÁN PRINCIPAL').html,'stock')).toContain('is-max');
  expect(chipClasses(card(html,'PUT WALL').html,'acceptance')).toContain('is-max');
 });
 it('handles real-shaped adjacent cent buckets with exact backend endpoint arithmetic',()=>{
  const f=fixture(),keys=[71000,71001,71002];
  const zones=keys.map((key,i)=>({lower:key/100,upper:(key+1)/100,firstMs:at-59000,lastMs:at-100,shares:(i+1)*100+30,buy:10,sell:(i+1)*100,unknown:20}));
  (f.cvd.executionEvidence!.windows[0] as unknown as {zones:typeof zones}).zones=zones;
  const prices=[710,710.005,710.01,710.02,710.03];
  const input:LevelActivityInput={symbol:'QQQ',active:true,publishedMs:at,cvd:f.cvd,auction:f.auction,optionRows:f.optionRows,expirations:[date],optionFresh:true,optionAtMs:at};
  const observed=readLevelActivities(prices,input,at);
  expect(prices.map(price=>observed.get(strikeKey(price)!)!.stockSales.value)).toEqual([100,100,200,300,null]);
  expect(observed.get(strikeKey(710.02)!)!.stockSales.max).toBe(true);
  expect(observed.get(strikeKey(710.01)!)!.stockSales.detail).toContain('$710.01–$710.02');
  expect(observed.get(strikeKey(710.03)!)!.stockSales.max).toBe(false);
 });
});
