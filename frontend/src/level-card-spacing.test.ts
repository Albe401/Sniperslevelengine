import {describe,it,expect} from 'vitest';
import {levelCardPositions,retainedCvdSpace} from './level-card-spacing';

it('retains the old CVD card footprint without rendering it in the web map',()=>{
 expect(retainedCvdSpace(494.47)).toBe(166); // 800px viewport: 706 -> 872px panel
 expect(retainedCvdSpace(270.98)).toBe(166); // 1500px viewport: 714 -> 880px panel
 expect(retainedCvdSpace(260)).toBe(180);    // 1440px viewport: 742 -> 922px panel
});
describe('filled level map',()=>{
 for(const heights of [[80,240,114,114,240,80],[100,130,120,90,80,110],[50,55],[60,60,60,60,60,60,60,60],[400,400,400,400],[]]){
  it(`fills columns without overlaps or clipping for ${heights}`,()=>{
   const available=800,p=levelCardPositions(heights,available);
   for(const column of [0,1]){
    const rows=p.filter(r=>r.column===column);if(!rows.length)continue;
    expect(rows[0].top).toBe(0);
    rows.forEach((r,i)=>{if(i)expect(r.top-rows[i-1].top-rows[i-1].height).toBeCloseTo(6);});
    const natural=rows.reduce((s,r)=>s+heights[p.indexOf(r)],0)+(rows.length-1)*6;
    expect(rows.at(-1)!.top+rows.at(-1)!.height).toBeCloseTo(Math.max(available,natural));
   }
   p.forEach((r,i)=>expect(r.height).toBeGreaterThanOrEqual(heights[i]));
  });
 }
 it('preserves natural card sizes when content exceeds the viewport',()=>{
  expect(levelCardPositions([400,400,400,400],600).every(p=>p.height===400)).toBe(true);
 });
 it('balances uneven cards so all content fits when possible',()=>{
  expect(Math.max(...levelCardPositions([115,340,142,154,115,340],685).map(p=>p.top+p.height))).toBeCloseTo(685);
 });
});
