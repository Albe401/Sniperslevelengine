import type {Block} from './investment-research';
import type {SavedBlocks,Side} from './overnight-types';
import {etDate,etHour,etMinute} from './overnight-types';

export interface BlockPressure {date:string;symbol:string;buy:number;sell:number;buyShares:number;sellShares:number;unknown:number;count:number;classified:number;coverage:number;buyPct:number|null;sellPct:number|null;dominance:Side;lastHour:Side;lastHourBuy:number;lastHourSell:number;lastHourShare:number;lastHourLargest:number;weightedPrice:number|null;premarketBuy:number;premarketSell:number;premarketUnknown:number;premarketCount:number;premarketDominance:Side;zones:{price:number;notional:number;side:Side}[];latestMs:number|null;reason:string}

export function verifiedBlockSide(b:Block):Side {
 if(!Number.isFinite(b.price)||b.price<=0||!Number.isFinite(b.notional)||b.notional<=0||!Number.isFinite(b.shares)||b.shares<=0||b.executionMs==null||b.quoteMs==null||b.bid==null||b.ask==null||!Number.isFinite(b.bid)||!Number.isFinite(b.ask)||b.bid<=0||b.ask<=b.bid||b.quoteMs>b.executionMs||b.executionMs-b.quoteMs>2000||(b.ask-b.bid)/((b.ask+b.bid)/2)>.01||!Array.isArray(b.conditions)||b.conditions.some(c=>![0,12,37].includes(c)))return 'PENDING';
 if(b.nbbo==='En ask'||b.nbbo==='Sobre ask')return 'BUY';
 if(b.nbbo==='En bid'||b.nbbo==='Bajo bid')return 'SELL';
 return 'PENDING';
}
const side=(buy:number,sell:number):Side=>buy+sell<=0?'PENDING':buy/(buy+sell)>=.55?'BUY':sell/(buy+sell)>=.55?'SELL':'NEUTRAL';
export function readBlockPressure(saved:SavedBlocks|null,symbol:string,date:string):BlockPressure {
 const empty:BlockPressure={date,symbol,buy:0,sell:0,buyShares:0,sellShares:0,unknown:0,count:0,classified:0,coverage:0,buyPct:null,sellPct:null,dominance:'PENDING',lastHour:'PENDING',lastHourBuy:0,lastHourSell:0,lastHourShare:0,lastHourLargest:0,weightedPrice:null,premarketBuy:0,premarketSell:0,premarketUnknown:0,premarketCount:0,premarketDominance:'PENDING',zones:[],latestMs:null,reason:'Sin bloques guardados del postcierre seleccionado'};
 if(!saved||saved.report.date!==date)return empty;
 let priceSum=0,lastPriceNotional=0;
 const zones=new Map<number,{price:number;notional:number;buy:number;sell:number}>();
 for(const {ticker,block} of saved.report.rows){const ms=block.executionMs;
  if(ticker!==symbol||ms==null||!Number.isFinite(block.notional)||block.notional<=0)continue;
  const postClose=etDate(ms)===date&&etHour(ms)>=16&&etHour(ms)<20;
  const premarket=etDate(ms)>date&&etHour(ms)>=4&&(etHour(ms)<9||etHour(ms)===9&&etMinute(ms)<30);
  if(!postClose&&!premarket)continue;
  const classified=verifiedBlockSide(block);empty.latestMs=Math.max(empty.latestMs??0,ms);
  if(premarket){empty.premarketCount++;if(classified==='BUY')empty.premarketBuy+=block.notional;else if(classified==='SELL')empty.premarketSell+=block.notional;else empty.premarketUnknown+=block.notional;continue;}
  empty.count++;
  if(classified==='BUY'){empty.buy+=block.notional;empty.buyShares+=block.shares;}else if(classified==='SELL'){empty.sell+=block.notional;empty.sellShares+=block.shares;}else empty.unknown+=block.notional;
  if(classified!=='PENDING'){priceSum+=block.price*block.notional;lastPriceNotional+=block.notional;}
  if(etHour(ms)===19){if(classified==='BUY')empty.lastHourBuy+=block.notional;if(classified==='SELL')empty.lastHourSell+=block.notional;if(classified!=='PENDING')empty.lastHourLargest=Math.max(empty.lastHourLargest,block.notional);}
  if(classified!=='PENDING'){const price=Math.round(block.price*4)/4,z=zones.get(price)??{price,notional:0,buy:0,sell:0};z.notional+=block.notional;if(classified==='BUY')z.buy+=block.notional;else z.sell+=block.notional;zones.set(price,z);}
 }
 empty.classified=empty.buy+empty.sell;const total=empty.classified+empty.unknown;empty.coverage=total?100*empty.classified/total:0;
 empty.buyPct=empty.classified?100*empty.buy/empty.classified:null;empty.sellPct=empty.buyPct==null?null:100-empty.buyPct;
 empty.dominance=empty.coverage>=50?side(empty.buy,empty.sell):'PENDING';
 empty.lastHour=side(empty.lastHourBuy,empty.lastHourSell);empty.weightedPrice=lastPriceNotional?priceSum/lastPriceNotional:null;
 empty.lastHourShare=empty.classified?100*(empty.lastHourBuy+empty.lastHourSell)/empty.classified:0;
 const premarketTotal=empty.premarketBuy+empty.premarketSell+empty.premarketUnknown;
 empty.premarketDominance=premarketTotal>0&&100*(empty.premarketBuy+empty.premarketSell)/premarketTotal>=50?side(empty.premarketBuy,empty.premarketSell):'PENDING';
 empty.zones=[...zones.values()].sort((a,b)=>b.notional-a.notional).slice(0,3).map(z=>({price:z.price,notional:z.notional,side:side(z.buy,z.sell)}));
 empty.reason=!empty.count?'Sin bloques del activo entre 16:00 y 20:00 ET':empty.coverage<50?'Menos de 50% del importe postcierre con dirección NBBO verificable':`Dirección estimada frente a NBBO · ${empty.count} bloques postcierre · no es flujo neto institucional`;
 return empty;
}
