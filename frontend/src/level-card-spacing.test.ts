import {describe,it,expect} from 'vitest';
import {levelCardPositions,levelOverviewHeight} from './level-card-spacing';

it('restores the original base and grows to show every complete panel without inner scrolling',()=>{
 expect(levelOverviewHeight(700,750,600)).toBe(824);
 expect(levelOverviewHeight(1012.4,800,950)).toBe(1013);
 expect(levelOverviewHeight(750,900,1100)).toBe(1100);
 expect(levelOverviewHeight(NaN,Infinity)).toBe(824);
});

it('fills the fixed rail to its lower edge without shrinking content',()=>{
 const heights=[180,90,120,130,80,180],p=levelCardPositions(heights,600);
 for(const column of [0,1]){
  const rows=p.filter(r=>r.column===column);
  expect(rows.at(-1)!.top+rows.at(-1)!.height).toBeCloseTo(600);
  rows.forEach((r,i)=>{if(i)expect(r.top-rows[i-1].top-rows[i-1].height).toBeCloseTo(6);});
 }
 p.forEach((r,i)=>expect(r.height).toBeGreaterThanOrEqual(heights[i]));
});

it('keeps the same cards on the same side when live text heights change',()=>{
 const first=levelCardPositions([180,90,120,130,80,180]);
 const columns=first.map(p=>p.column),changed=levelCardPositions([240,90,160,100,80,210],800,columns);
 expect(changed.map(p=>p.column)).toEqual(columns);
 for(const column of [0,1]){const rows=changed.filter(p=>p.column===column);expect(rows.at(-1)!.top+rows.at(-1)!.height).toBeCloseTo(800);}
});
describe('compact level map',()=>{
 for(const heights of [[80,240,114,114,240,80],[100,130,120,90,80,110],[50,55],[60,60,60,60,60,60,60,60],[400,400,400,400],[]]){
  it(`keeps natural card heights and close columns without overlaps for ${heights}`,()=>{
   const p=levelCardPositions(heights);
   for(const column of [0,1]){
    const rows=p.filter(r=>r.column===column);if(!rows.length)continue;
    expect(rows[0].top).toBe(0);
    rows.forEach((r,i)=>{if(i)expect(r.top-rows[i-1].top-rows[i-1].height).toBeCloseTo(6);});
    const natural=rows.reduce((s,r)=>s+heights[p.indexOf(r)],0)+(rows.length-1)*6;
    expect(rows.at(-1)!.top+rows.at(-1)!.height).toBeCloseTo(natural);
   }
   p.forEach((r,i)=>expect(r.height).toBe(heights[i]));
  });
 }
 it('preserves small and tall content without stretching or clipping',()=>{
  expect(levelCardPositions([50,55]).map(p=>p.height)).toEqual([50,55]);
  expect(levelCardPositions([400,400,400,400]).every(p=>p.height===400)).toBe(true);
 });
 it('balances uneven cards so all content fits when possible',()=>{
  expect(Math.max(...levelCardPositions([115,340,142,154,115,340]).map(p=>p.top+p.height))).toBeLessThanOrEqual(685);
 });
});
