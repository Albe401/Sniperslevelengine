/** Licensed Gexbot Classic prices from the same saved asset frame. */
export interface GexbotDominants {
 source:string;symbol:string;category:string;generatedMs:number;
 positiveOi:number|null;negativeOi:number|null;positiveVolume:number|null;negativeVolume:number|null;zeroGamma:number|null;
}
export interface GexbotMark {price:number;label:string;side:'positive'|'negative'|'zero';weight:'volume'|'oi'}
export function gexbotMarks(value:GexbotDominants|undefined,symbol:string,capturedMs:number,now:number):GexbotMark[] {
 if(!value||value.source!=='Gexbot Classic'||value.symbol!==symbol||value.category!=='gex_full'
  ||!Number.isSafeInteger(now)||!Number.isSafeInteger(value.generatedMs)||!Number.isSafeInteger(capturedMs)||value.generatedMs<=0||capturedMs<=0
  ||value.generatedMs>capturedMs||capturedMs-value.generatedMs>600000
  ||now<capturedMs||now-value.generatedMs>1200000)return [];
 const rows:[number|null,string,'positive'|'negative'|'zero','volume'|'oi'][]=[
  [value.positiveVolume,'GEX + VOL','positive','volume'],
  [value.negativeVolume,'GEX − VOL','negative','volume'],
  [value.positiveOi,'GEX + OI','positive','oi'],
  [value.negativeOi,'GEX − OI','negative','oi'],
  [value.zeroGamma,'ZERO GAMMA VOL','zero','volume'],
 ];
 return rows.filter(([price])=>typeof price==='number'&&Number.isFinite(price)&&price>0)
  .map(([price,label,side,weight])=>({price:price!,label,side,weight}));
}
