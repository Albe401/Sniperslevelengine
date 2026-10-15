import type {MarketEvidence} from './market-evidence';
import {escapeHtml as esc} from './model';
export function sharedLevelDetail(read:MarketEvidence|undefined,strike:number|undefined):string {
 if(!read)return '';const level=read.levels.find(l=>l.price===strike),e=level?.evidence;
 const n=(v:number|null|undefined)=>v==null?'pendiente':new Intl.NumberFormat('es',{maximumFractionDigits:2,notation:'compact'}).format(v);
 return `<p>${esc(read.detail)}</p>${e?`<p>${esc(e.detail)}</p><p>OI ${n(e.oi)} · ΔOI ${n(e.oiChange)} · volumen ${n(e.volume)} · vol/OI ${n(e.volumeOi)} · delta absoluta ${n(e.dex)} · GEX ${n(e.gex)} · participación OI ${n(e.oiShare)}% / gamma ${n(e.gammaShare)}%.</p>`:'<p>Concentración del nivel pendiente: no se sustituye por cero.</p>'}`;
}
