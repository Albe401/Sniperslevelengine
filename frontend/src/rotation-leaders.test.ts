import {it,expect} from 'vitest';
import {rotationLeaders,rotationCards} from './rotation-leaders';
import type {Scanner} from './scanner';
const now=Date.parse('2026-09-25T15:00:00Z');
function fixture():Scanner {return {connected:true,rows:[],top:[],total:10,historicalReady:10,sectorActivity:{barEndMs:now,sectors:[],rotation:[{minutes:10,ready:true,reason:'ok',coveragePct:100,sectors:[{name:'TECH',state:'GANANDO CONCENTRACIÓN'},{name:'ENERGY',state:'PERDIENDO CONCENTRACIÓN'}].map(s=>({...s,currentUsd:100,previousUsd:50,changeUsd:50,sharePct:50,changePp:5,observed:5,expected:5})),leaders:Array.from({length:7},(_,i)=>({symbol:'A'+i,sector:'TECH',currentUsd:100+i,previousUsd:50,changeUsd:50+i}))}]}};}
it('ranks five by dollar increase inside gaining sectors and renders leader',()=>{const d=fixture(),r=rotationLeaders(d,true,now,now,10);expect(r.rows.map(a=>a.symbol)).toEqual(['A6','A5','A4','A3','A2']);expect(rotationCards(d,r,10)).toContain('LÍDER');expect(rotationLeaders(d,true,now,now,10,'ENERGY').rows).toEqual([]);});
it('revokes stale disconnected future incomplete and unpaired signals',()=>{
 for(const patch of ['disconnect','stale','future','coverage','unpaired','missing']){const d=fixture();if(patch==='disconnect')d.connected=false;if(patch==='stale')d.sectorActivity!.barEndMs=now-120001;if(patch==='future')d.sectorActivity!.barEndMs=now+1;if(patch==='coverage')d.sectorActivity!.rotation![0].coveragePct=79;if(patch==='unpaired')d.sectorActivity!.rotation![0].sectors.pop();if(patch==='missing')d.sectorActivity!.rotation![0].leaders=undefined;expect(rotationLeaders(d,true,now,now,10).rows).toEqual([]);}
 expect(rotationLeaders(fixture(),true,now-4000,now,10).rows).toEqual([]);
 expect(rotationLeaders(fixture(),true,now,now,30).rows).toEqual([]);
});
it('excludes wrong sector, negative changes, duplicates and invalid amounts',()=>{const d=fixture(),w=d.sectorActivity!.rotation![0];w.leaders=[{symbol:'BAD',sector:'ENERGY',currentUsd:999,previousUsd:0,changeUsd:999},{symbol:'NAN',sector:'TECH',currentUsd:NaN,previousUsd:0,changeUsd:99},{symbol:'LOSS',sector:'TECH',currentUsd:2,previousUsd:5,changeUsd:-3},...Array(2).fill({symbol:'DUP',sector:'TECH',currentUsd:9,previousUsd:1,changeUsd:8})];expect(rotationLeaders(d,true,now,now,10).rows).toEqual([]);});
