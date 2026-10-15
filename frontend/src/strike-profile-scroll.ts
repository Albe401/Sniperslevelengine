/** Navigation only: never changes candles, GEX, OI or the selected instrument. */
export function scrollStrikeCenter(strikes:number[],center:number|undefined,reference:number,delta:number){
 if(!strikes.length||!Number.isFinite(delta)||delta===0)return center;
 const values=[...new Set(strikes.filter(s=>Number.isFinite(s)&&s>0))].sort((a,b)=>a-b);
 if(!values.length)return undefined;
 const anchor=Number.isFinite(center)?center!:reference;
 let index=values.reduce((best,v,i)=>Math.abs(v-anchor)<Math.abs(values[best]-anchor)?i:best,0);
 const step=Math.min(8,Math.max(1,Math.round(Math.abs(delta)/40)));
 index=Math.max(0,Math.min(values.length-1,index-Math.sign(delta)*step));
 return values[index];
}
export function strikeProfileWindow(strikes:number[],center:number,height:number){
 const values=[...new Set(strikes.filter(s=>Number.isFinite(s)&&s>0))].sort((a,b)=>a-b);
 if(!values.length||!Number.isFinite(center)||height<=110)return [];
 const anchor=values.reduce((best,v,i)=>Math.abs(v-center)<Math.abs(values[best]-center)?i:best,0);
 const mid=80+(height-115)/2;
 return values.map((strike,i)=>({strike,y:mid+(anchor-i)*28})).filter(r=>r.y>=80&&r.y<=height-35);
}
