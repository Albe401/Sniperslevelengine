import {describe,it,expect} from 'vitest';
import {levelDynamicsPanel,type LevelDynamics,type DynamicLevel} from './level-dynamics';
const now=Date.UTC(2026,8,28,15);
const level={name:'PUT WALL',strike:490,low:489,high:491,provisional:true,premium:null,callBuy:null,callSell:null,putBuy:null,putSell:null,unknown:null,flowComplete:false,classifiedPct:null,oi:1000,oiChange:null,oiReference:'2026-09-25',dailyVolume:400,strength:65,category:'HIGH',coverage:75,change:null,trend:'ESTABLECIENDO BASE',pull:null,pullReason:'Sin evidencia',components:[]} satisfies DynamicLevel;
const fixture:LevelDynamics={symbol:'QQQ',at:now,available:true,preferred:null,reason:'Zonas observadas',levels:[level]};
describe('dynamic levels',()=>{
 it('keeps the three colored cards without invented values while waiting',()=>{for(const report of [undefined,{...fixture,available:false},{...fixture,at:now-3001}]){const html=levelDynamicsPanel(report,'QQQ',now);for(const tone of ['put','magnet','call'])expect(html).toContain(`dynamic-level ${tone}`);expect(html.match(/class="dynamic-strike">—/g)).toHaveLength(3);expect(html).not.toContain('$490');expect(html).not.toContain('0%');}});
 it('separates observed premiums, daily OI and model scores',()=>{const html=levelDynamicsPanel(fixture,'QQQ',now);expect(html).toContain('Δ OI entre días');expect(html).toContain('Estructura provisional');expect(html).toContain('Flujo parcial');expect(html).toContain('no identifican apertura/cierre ni capital neto');expect(html).not.toContain('$0');expect(html).toContain('Sin atracción dominante verificable');});
 it('rejects foreign, future, stale and unavailable reports',()=>{for(const r of [{...fixture,symbol:'SPY'},{...fixture,at:now+1},{...fixture,at:now-3001},{...fixture,available:false}])expect(levelDynamicsPanel(r,'QQQ',now)).not.toContain('$490');});
 it('escapes model text and keeps zero distinct from missing',()=>{const html=levelDynamicsPanel({...fixture,levels:[{...level,name:'<script>',premium:0}]},'QQQ',now);expect(html).not.toContain('<script>');expect(html).toContain('&lt;script&gt;');expect(html).toContain('$0');});
});
