export interface AuctionPopulationComparison {
 version:string;
 consolidated?:{pocDiagnostics?:{shares:number;largestTradeShares:number;largestTradeConcentrationPct:number|null}};
 continuous?:{symbol:string;asOfMs:number;pocLow:number;pocHigh:number;val:number;vah:number;totalVolume:number}|null;
 separated?:{shares:number};
}
/** Descriptive quantities only. Missing variant never becomes a substitute POC. */
export function auctionPopulationDetail(symbol:string,asOfMs:number,p?:AuctionPopulationComparison):string {
 if(!p||p.version!=='amt-continuous-partition-20261013-v1')return '';
 const n=(v:number)=>v.toFixed(2),messages=['POC del volumen consolidado.'];
 const q=p.consolidated?.pocDiagnostics;
 if(q&&[q.shares,q.largestTradeShares].every(v=>Number.isFinite(v)&&v>0)&&q.largestTradeShares<=q.shares){
  messages.push(`Mayor print: ${n(q.largestTradeShares)} acciones · ${n(100*q.largestTradeShares/q.shares)}% del volumen de este nivel; concentración, no aceptación ni probabilidad.`);
 }
 const c=p.continuous;
 if(c&&c.symbol===symbol&&c.asOfMs===asOfMs&&[c.pocLow,c.pocHigh,c.val,c.vah,c.totalVolume].every(v=>Number.isFinite(v)&&v>0)&&c.pocHigh>c.pocLow&&c.vah>c.val){
  messages.push(`Perfil continuo experimental: POC ${n(c.pocLow)}–${n(c.pocHigh)} · VAL ${n(c.val)} · VAH ${n(c.vah)}.`);
 }else messages.push('Perfil continuo pendiente; no se sustituye con el consolidado.');
 if(p.separated&&Number.isFinite(p.separated.shares)&&p.separated.shares>=0)messages.push(`Volumen separado por condiciones: ${n(p.separated.shares)} acciones. Conservado en el consolidado; sin filtro por tamaño o TRF.`);
 return messages.join(' ');
}
