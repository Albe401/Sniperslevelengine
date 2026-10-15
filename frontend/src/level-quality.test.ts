import {describe,it,expect} from 'vitest';
import {levelQuality,type LevelQuality} from './level-quality';
const q=(strike:number,patch:Partial<LevelQuality>={}):LevelQuality=>({strike,total:100,covered:100,oiCoverage:100,missingOi:0,missingVolume:0,missingGamma:0,...patch});
describe('local quality',()=>{
 it('does not contaminate a complete wall with a remote missing contract',()=>{expect(levelQuality([q(742),q(800,{covered:99,missingOi:1,oiCoverage:null})],742,742,true).provisional).toBe(false)});
 it('balances small known omissions without certifying completeness',()=>{const r=levelQuality([q(742,{covered:99,missingVolume:1,oiCoverage:99.8})],742,742,true);expect(r.provisional).toBe(false);expect(r.detail).toContain('no completa')});
 it('keeps unknown or material local omissions provisional',()=>{for(const patch of [{covered:99,missingOi:1,oiCoverage:null},{covered:99,oiCoverage:70},{covered:80,oiCoverage:99}])expect(levelQuality([q(742,patch)],742,742,true).provisional).toBe(true)});
 it('checks the whole vacuum range and fails closed for missing telemetry',()=>{expect(levelQuality([q(741),q(742,{covered:0,oiCoverage:0})],741,742,true).provisional).toBe(true);expect(levelQuality(undefined,742,742,true).provisional).toBe(true)});
});
