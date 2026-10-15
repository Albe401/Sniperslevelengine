export interface Candle {minute:number;o:number;h:number;l:number;c:number;v:number}
export function aggregateCandles(rows:Candle[],interval:number):Candle[]{
  const result:Candle[]=[];
  for(const row of rows.slice().sort((a,b)=>a.minute-b.minute)){
    const minute=Math.floor(row.minute/interval)*interval,last=result.at(-1);
    if(last&&last.minute===minute){last.h=Math.max(last.h,row.h);last.l=Math.min(last.l,row.l);last.c=row.c;last.v+=row.v;}
    else result.push({...row,minute});
  }
  return result;
}
