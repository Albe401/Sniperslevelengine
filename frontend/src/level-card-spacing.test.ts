import {describe,it,expect} from 'vitest';
import {levelCardPositions} from './level-card-spacing';
describe('fixed-map internal placement',()=>{
 for(const heights of [[100,300,140,140,140,260,100],[64,64,64,64,64,64],[300,300,300,300,300,300,300,300]]){
  it(`keeps balanced counts and nonoverlapping cards for ${heights.join(',')}`,()=>{
   const positions=levelCardPositions(heights),counts=[0,0],bottoms=[0,0];
   positions.forEach((p,i)=>{expect(p.top).toBeGreaterThanOrEqual(bottoms[p.column]);bottoms[p.column]=p.top+heights[i]+12;counts[p.column]++;});
   expect(Math.abs(counts[0]-counts[1])).toBeLessThanOrEqual(1);
  });
 }
});
