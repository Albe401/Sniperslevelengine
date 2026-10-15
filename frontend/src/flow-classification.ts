type PremiumWindow = {
  event_count:number; premium:number; premium_complete:boolean;
  buckets?:Record<string,{premium:number}>;
};
const labels=['CALL BUY','CALL SELL','PUT BUY','PUT SELL','UNKNOWN'];

// This measures inferred-side coverage of observed premium, not coverage of
// the entire options market or certainty about institutional intent.
export function classifiedPremiumPercent(window:PremiumWindow|undefined,fresh:boolean):number|null {
  if(!fresh||!window?.premium_complete||!Number.isInteger(window.event_count)||window.event_count<=0||!Number.isFinite(window.premium)||window.premium<=0)return null;
  const amounts=labels.map(label=>window.buckets?.[label]?.premium);
  if(amounts.some(n=>n==null||!Number.isFinite(n)||n<0))return null;
  const known=amounts.slice(0,4).reduce<number>((sum,n)=>sum+n!,0);
  const total=known+amounts[4]!;
  if(!Number.isFinite(total)||total<=0||Math.abs(total-window.premium)>window.premium*1e-8)return null;
  return Math.min(100,Math.max(0,known/total*100));
}
export function classificationNote(window:PremiumWindow|undefined,fresh:boolean):string {
  const percent=classifiedPremiumPercent(window,fresh);
  if(percent==null)return 'Prima clasificada: —';
  const value=percent>0&&percent<0.1?'<0.1':percent<100&&percent>99.9?'>99.9':percent.toFixed(1);
  return `Prima clasificada: ${value}% de la observada · lado inferido`;
}
