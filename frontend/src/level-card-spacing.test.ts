import {describe,it,expect} from 'vitest';
import {levelCardPositions} from './level-card-spacing';
describe('adaptive level rail',()=>{
 it('keeps compact gaps without stretching cards',()=>{
  const heights=[100,130,120,90,80,110],p=levelCardPositions(heights,600);
  for(const column of [0,1]){const rows=p.filter(r=>r.column===column);expect(rows[0].top).toBe(0);rows.forEach((row,i)=>{if(i)expect(row.top-rows[i-1].top-rows[i-1].height).toBeCloseTo(12);});}
  p.forEach((r,i)=>expect(r.height).toBe(heights[i]));
 });
 it('prevents a large hole in an uneven NVDA-like layout',()=>{
  const p=levelCardPositions([80,240,114,114,240,80],670);
  for(const column of [0,1]){const rows=p.filter(r=>r.column===column); rows.forEach((row,i)=>{if(i)expect(row.top-rows[i-1].top-rows[i-1].height).toBeLessThanOrEqual(12);});}
 });
 it('balances occupied height instead of forcing equal card counts',()=>{
  const p=levelCardPositions([300,60,60,60,60,60],400);
  expect(p.filter(r=>r.column===0)).toHaveLength(1);
  expect(p.filter(r=>r.column===1)).toHaveLength(5);
  expect(Math.max(...p.map(r=>r.top+r.height))).toBeLessThanOrEqual(400);
 });
 it('moves later levels when another card grows',()=>{
  const a=levelCardPositions([100,100,100,100,100,100],600),b=levelCardPositions([250,100,100,100,100,100],600);
  expect(b.some((r,i)=>r.top!==a[i].top||r.column!==a[i].column)).toBe(true);
 });
 for(const heights of [[100,300,140,140,140,260,100],[64,64,64,64,64,64],[400,400,400,400]]){
  it(`preserves content and avoids overlaps: ${heights}`,()=>{
   const p=levelCardPositions(heights,600),bottoms=[-6,-6];
   p.forEach((r,i)=>{expect(r.top).toBeGreaterThanOrEqual(bottoms[r.column]+6-1e-8);expect(r.height).toBe(heights[i]);bottoms[r.column]=r.top+r.height;});
  });
 }
});
