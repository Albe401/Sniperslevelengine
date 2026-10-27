/** Candle availability only. Freshness, session dates and emitted events keep the evaluator clock. */
export function publicationCandleClock(publishedMs:unknown,evaluatedMs:number):number {
 const clock=(n:unknown):n is number=>typeof n==='number'&&Number.isSafeInteger(n)&&n>0&&n<=8640000000000000;
 return clock(publishedMs)&&clock(evaluatedMs)?Math.min(publishedMs,evaluatedMs):NaN;
}
