/** Options strike identity at 0.001 USD precision; rejects unrepresentable values. */
export function strikeKey(value:number):number|null {
 const scaled=value*1000,key=Math.round(scaled);
 return Number.isFinite(value)&&value>0&&Number.isSafeInteger(key)&&Math.abs(scaled-key)<=0.0001?key:null;
}
export const sameStrike=(a:number,b:number)=>strikeKey(a)!=null&&strikeKey(a)===strikeKey(b);
