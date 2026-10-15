import type {ContractWindow} from './flow-coverage';
const number=(n:number|null)=>n==null?'—':Intl.NumberFormat('es',{maximumFractionDigits:2}).format(n);
export function optionSideVolume(rows:ContractWindow[],expirations:string[],kind:'CALL'|'PUT',minutes:number,fresh:boolean):number|null {
 if(!fresh||!expirations.length)return null;
 let total=0;const tickers=new Set<string>();
 for(const expiration of new Set(expirations)){
  const selected=rows.filter(r=>r.expiration===expiration&&r.kind.toUpperCase()===kind&&r.minutes===minutes);
  if(!selected.length)return null;
  for(const row of selected){
   if(tickers.has(row.ticker)||!row.covered||row.contracts==null||!Number.isFinite(row.contracts)||row.contracts<0)return null;
   tickers.add(row.ticker);total+=row.contracts;
  }
 }
 return total;
}
export function callPutVolumePanel(rows:ContractWindow[],expirations:string[],fresh:boolean):string {
 return `<section class="panel call-put-volume"><span class="eyebrow">OPCIONES · CONTRATOS OBSERVADOS</span><h3>Volumen CALL / PUT</h3>${[1,5].map(minutes=>{
 const call=optionSideVolume(rows,expirations,'CALL',minutes,fresh),put=optionSideVolume(rows,expirations,'PUT',minutes,fresh);
 const share=call!=null&&put!=null&&call+put>0?call/(call+put)*100:null;
 return `<div class="call-put-window"><h4>Últim${minutes===1?'o minuto':'os 5 minutos'}</h4><div class="call-put-values"><div class="call"><span>CALL</span><strong>${number(call)}</strong></div><div class="put"><span>PUT</span><strong>${number(put)}</strong></div></div>${share!=null?`<div class="call-put-bar" aria-label="CALL ${share.toFixed(1)} %, PUT ${(100-share).toFixed(1)} %"><span style="width:${share}%"></span></div><p>CALL ${share.toFixed(1)} % · PUT ${(100-share).toFixed(1)} %</p>`:'<p>Sin proporción disponible</p>'}<small>${call==null||put==null?'Cobertura pendiente: —':call+put===0?'Ventanas observadas sin operaciones':'Ventanas observadas'}</small></div>`;
 }).join('')}<p class="panel-caption">Solo contratos suscritos de los vencimientos DAY del activo; no es el total del mercado. CALL/PUT identifica el tipo de opción, no compras/ventas ni dirección. Las ventanas se solapan; no se suman.</p></section>`;
}
