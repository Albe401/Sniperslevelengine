import {escapeHtml as esc} from './model';
import type {GammaLevel} from './gamma-live-model';
export interface HeatRow {strike:number;call:number|null;put:number|null;y:number}
const compact=new Intl.NumberFormat('es',{notation:'compact',maximumFractionDigits:1});
const contracts=new Intl.NumberFormat('es-ES',{maximumFractionDigits:0});
const valid=(n:number|null)=>n!=null&&Number.isFinite(n)&&n>=0?n:null;
export function strikeHeatmap(rows:HeatRow[],height:number,metric:string,levels:GammaLevel[],complete:boolean,peak:number,headerY=80):string {
 const scale=Number.isFinite(peak)&&peak>0?peak:1;
 const main=levels.find(l=>l.id==='main'),wall=levels.find(l=>l.id==='call');
 const n=(v:number|null)=>v==null?'—':metric==='oi'||metric==='volume'?contracts.format(v):compact.format(v);
 const bars=rows.map(r=>{
  const call=valid(r.call),put=valid(r.put),reference=r.strike===main?.strike,wallHere=r.strike===wall?.strike;
  const bar=(v:number|null,isCall:boolean)=>{if(v==null)return '';const ratio=Math.min(1,v/scale),width=ratio*112,x=isCall?232:158-width,color=isCall?'#17d6a2':'#ff6273';return `<rect x="${isCall?232:46}" y="${r.y-9}" width="112" height="18" rx="3" fill="${color}" opacity=".16"/><rect x="${x}" y="${r.y-9}" width="${width}" height="18" rx="3" fill="${color}" opacity=".95"/>`;};
  return `<g><title>Strike ${r.strike.toFixed(2)} · ${esc(metric.toUpperCase())} · PUT ${n(put)} · CALL ${n(call)} · ${complete?'universo completo':'muestra parcial'}${wallHere?' · Muro gamma CALL':''}${reference?' · Referencia gamma':''}</title>${wallHere||reference?`<rect x="5" y="${r.y-11}" width="380" height="22" rx="4" fill="none" stroke="${wallHere?'#f2cf63':'#b48aff'}"/>`:''}${bar(put,false)}${bar(call,true)}<text x="6" y="${r.y+4}" paint-order="stroke" stroke="#061326" stroke-width="2" fill="#ff6273" font-size="12" font-weight="700">${n(put)}</text><text x="386" y="${r.y+4}" text-anchor="end" paint-order="stroke" stroke="#061326" stroke-width="2" fill="#17d6a2" font-size="12" font-weight="700">${n(call)}</text><text x="195" y="${r.y+4}" text-anchor="middle" fill="#ffffff" font-size="13" font-weight="700">${r.strike.toFixed(2)}</text></g>`;
 }).join('');
 const label=(l:GammaLevel|undefined)=>l?.strike==null?'—':l.strike.toFixed(2);
 return `<text x="102" y="${headerY}" text-anchor="middle" fill="#ff6273" font-size="10">PUT</text><text x="287" y="${headerY}" text-anchor="middle" fill="#17d6a2" font-size="10">CALL</text>${bars||`<text x="195" y="${height/2}" text-anchor="middle" fill="#a6c7e4" font-size="11">Sin ${esc(metric.toUpperCase())} verificable en este rango</text>`}<rect x="5" y="${height-88}" width="380" height="57" rx="5" fill="#09192c" fill-opacity=".8" stroke="#294260"/><text x="195" y="${height-73}" text-anchor="middle" fill="#a9c6e5" font-size="10">${metric==='oi'?'OI · CONTRATOS ESTÁNDAR ABIERTOS':'DEALERS · POSICIONAMIENTO ESTIMADO'}</text><text x="16" y="${height-57}" fill="#f2cf63" font-size="10">${metric==='oi'?'Suma de los vencimientos incluidos':'Muro gamma CALL '+label(wall)}</text><text x="16" y="${height-43}" fill="#b48aff" font-size="10">${metric==='oi'?'Escala común CALL / PUT · — dato no verificable':'Referencia gamma '+label(main)+' · GEX / OI'}</text>`;
}
