import {describe,it,expect} from 'vitest';
import {levelCardPositions} from './level-card-spacing';
describe('fixed-map internal placement',()=>{
 it('fills each column inside the existing height without growing the map',()=>{
  const heights=[100,130,120,90,80,110],positions=levelCardPositions(heights,600);
  for(const column of [0,1]){
   const rows=positions.filter(p=>p.column===column);
   expect(rows[0].top).toBe(0);
   expect(rows.at(-1)!.top+rows.at(-1)!.height).toBeCloseTo(600);
   rows.forEach((p,i)=>{if(i)expect(p.top-rows[i-1].top-rows[i-1].height).toBeCloseTo(12);});
  }
  positions.forEach((p,i)=>expect(p.height).toBeGreaterThanOrEqual(heights[i]));
 });
 it('preserves tall content and permits internal scrolling instead of compressing it',()=>{
  const positions=levelCardPositions([400,400,400,400],600);
  expect(Math.max(...positions.map(p=>p.top+p.height))).toBe(812);
  expect(positions.every(p=>p.height===400)).toBe(true);
 });
 for(const heights of [[100,300,140,140,140,260,100],[64,64,64,64,64,64],[300,300,300,300,300,300,300,300]]){
  it(`keeps balanced counts and nonoverlapping cards for ${heights.join(',')}`,()=>{
   const positions=levelCardPositions(heights),counts=[0,0],bottoms=[0,0];
   positions.forEach((p,i)=>{expect(p.top).toBeGreaterThanOrEqual(bottoms[p.column]);bottoms[p.column]=p.top+heights[i]+12;counts[p.column]++;});
   expect(Math.abs(counts[0]-counts[1])).toBeLessThanOrEqual(1);
  });
 }
});
