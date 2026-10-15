import {price} from './model';
/** Both profiles share the price series projection; gamma is the signed GEX
 * exposure divided by the engine's spot² × 1% factor (includes multiplier/OI). */
export function routeProfileSvg(rows:{strike:number;net_gex:number}[],spot:number|undefined,height:number,coordinate:(p:number)=>number|null,gamma=false):string {
 if(spot==null||!Number.isFinite(spot)||spot<=0||!Number.isFinite(height)||height<=0)return '';
 const divisor=gamma?spot*spot*.01:1;
 if(!Number.isFinite(divisor)||divisor<=0)return '';
 const valid=rows.filter(r=>Number.isFinite(r.strike)&&r.strike>0&&Number.isFinite(r.net_gex)&&spot!=null&&Math.abs(r.strike/spot-1)<=.05);
 const peak=Math.max(1e-12,...valid.map(r=>Math.abs(r.net_gex/divisor)));
 return `<line x1="65" x2="65" y1="35" y2="${height}" stroke="#b4c8bc30" stroke-dasharray="3 4"/>${valid.map(r=>{const y=coordinate(r.strike);if(y==null||!Number.isFinite(y)||y<35||y>height-25)return '';const v=r.net_gex/divisor,size=Math.abs(v)/peak*(v>=0?120:45),color=gamma?(v>=0?'#26f46b':'#ff4958'):(v>=0?'#22dce8':'#ba51e6');return `<g><title>Strike ${price(r.strike)} · ${gamma?'Gamma ponderada firmada':'GEX USD / 1%'} ${v.toLocaleString('es')}</title><text x="3" y="${y-4}" fill="#8ba6bc" font-size="8">${price(r.strike)}</text><rect x="${v>=0?65:65-size}" y="${y}" width="${size}" height="2.5" fill="${color}"/></g>`;}).join('')}`;
}
