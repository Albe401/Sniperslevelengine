export interface LevelQuality {strike:number;total:number;covered:number;oiCoverage:number|null;missingOi:number;missingVolume:number;missingGamma:number}
/** Display quality only: never authorizes a signal or certifies dealer inventory. */
export function levelQuality(rows:LevelQuality[]|undefined,low:number,high=low,chainPartial=false){
 const local=rows?.filter(q=>q.strike>=low&&q.strike<=high)??[];
 const complete=local.length>0&&local.every(q=>q.total>0&&q.covered===q.total);
 // Both count and known OI must support the reading. Unknown OI cannot be outweighed.
 const strong=local.length>0&&local.every(q=>q.total>0&&q.covered/q.total>=.95&&q.missingOi===0&&q.oiCoverage!=null&&Number.isFinite(q.oiCoverage)&&q.oiCoverage>=95);
 // Scores are normalized against the global population, even at a complete local strike.
 const provisional=chainPartial;
 const detail=local.length?local.map(q=>`${q.strike}: ${q.covered}/${q.total} series completas; cobertura OI ${q.oiCoverage==null?'desconocida':q.oiCoverage.toFixed(1)+'%'}; faltan OI ${q.missingOi}, volumen ${q.missingVolume}, gamma ${q.missingGamma}.`).join(' '):'Cobertura local no disponible.';
 return {provisional,localComplete:complete,localStrong:strong,detail:detail+(chainPartial?' Cadena parcial: fuerza estimada respecto al subconjunto disponible; no confirma defensa.':'')+(strong&&!complete?' Cobertura local alta, no completa.':'')};
}
