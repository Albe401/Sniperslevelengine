/** Licensed Gexbot Classic prices from the same saved asset frame. */
export interface GexbotDominants {
 source:string;symbol:string;category:string;generatedMs:number;
 positiveOi:number;negativeOi:number;positiveVolume:number;negativeVolume:number;zeroGamma:number;
}
export interface GexbotMark {price:number;label:string;side:'positive'|'negative';weight:'volume'|'oi'}
export function gexbotMarks(value:GexbotDominants|undefined,symbol:string,capturedMs:number,now:number):GexbotMark[] {
 if(!value||value.source!=='Gexbot Classic'||value.symbol!==symbol||value.category!=='gex_full'
  ||!Number.isSafeInteger(value.generatedMs)||!Number.isSafeInteger(capturedMs)
  ||value.generatedMs>capturedMs||capturedMs-value.generatedMs>600000
  ||now<capturedMs||now-value.generatedMs>1200000)return [];
 const rows:[number,string,'positive'|'negative','volume'|'oi'][]=[
  [value.positiveVolume,'GEX + VOL','positive','volume'],
  [value.negativeVolume,'GEX − VOL','negative','volume'],
  [value.positiveOi,'GEX + OI','positive','oi'],
  [value.negativeOi,'GEX − OI','negative','oi'],
 ];
 return rows.filter(([price])=>typeof price==='number'&&Number.isFinite(price)&&price>0)
  .map(([price,label,side,weight])=>({price,label,side,weight}));
}
