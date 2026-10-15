import {it,expect} from 'vitest';
import {ivRank} from './iv-rank';
it('matches the supplied platform example',()=>expect(ivRank(36.58,27.6,53.5).rank).toBeCloseTo(34.67181467));
it('does not fabricate rank from missing or malformed history',()=>{for(const args of [[null,20,50],[30,null,50],[30,20,null],[NaN,20,50],[30,50,20],[30,20,20]] as [number|null,number|null,number|null][]){expect(ivRank(...args).rank).toBeNull();}});
it('includes new extrema in the current rolling range',()=>{expect(ivRank(60,20,50).rank).toBe(100);expect(ivRank(10,20,50).rank).toBe(0);});
it('classifies relative position without treating IV magnitude as rank',()=>{expect(ivRank(21,20,80).label).toContain('BAJA');expect(ivRank(75,20,80).label).toContain('ALTA');expect(ivRank(50,20,80).rank).toBe(50);});
