/** Rank of one consistently constructed IV series over 52 calendar weeks. */
export interface IvRankRead {rank:number|null;label:string;reason:string}
export function ivRank(current:number|null,low:number|null,high:number|null):IvRankRead {
 const valid=(n:number|null):n is number=>n!=null&&Number.isFinite(n)&&n>0;
 if(!valid(current)||!valid(low)||!valid(high))return {rank:null,label:'HISTORIAL PENDIENTE',reason:'Faltan IV actual, mínima o máxima de 52 semanas de la misma serie'};
 if(high<=low)return {rank:null,label:'RANGO NO VÁLIDO',reason:'La máxima debe superar la mínima; un rango constante no permite calcular IV Rank'};
 // Include today's observation in the window if it establishes a new extreme.
 const minimum=Math.min(low,current),maximum=Math.max(high,current);
 const rank=100*(current-minimum)/(maximum-minimum);
 return {rank,label:rank<33.3333333333?'IV RELATIVAMENTE BAJA':rank>66.6666666667?'IV RELATIVAMENTE ALTA':'IV EN RANGO INTERMEDIO',reason:'Posición en el rango de 52 semanas; bandas visuales de tres tercios, no recomendación de compra o venta'};
}
