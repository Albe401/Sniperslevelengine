/** Shared glyph painter only: provider and own-source calculations stay separate. */
export interface DominantMark {side:string;price:number;bar:number;at:number;fraction:number;provisional?:boolean}
export function paintDominantMarks(ctx:CanvasRenderingContext2D,marks:DominantMark[],width:number,height:number,xForBar:(t:number)=>number|null,barWidth:number,priceY:(p:number)=>number|null,legend:string){
 ctx.save();ctx.beginPath();ctx.rect(0,0,width,height);ctx.clip();ctx.lineCap='round';
 for(const m of marks){const anchor=xForBar(m.bar),y=priceY(m.price);if(anchor==null||y==null||!Number.isFinite(anchor)||!Number.isFinite(y))continue;const x=anchor-barWidth/2+m.fraction*barWidth;if(x<0||x>width||y<0||y>height)continue;
  ctx.globalAlpha=m.provisional?.45:1;ctx.setLineDash(m.provisional?[2,2]:[]);
  ctx.beginPath();ctx.moveTo(x-6,y);ctx.lineTo(x+6,y);ctx.strokeStyle='#061326';ctx.lineWidth=7;ctx.stroke();
  ctx.strokeStyle=m.side==='positive'?'#5effa1':'#ff6d8c';ctx.lineWidth=3;ctx.stroke();
 }
 ctx.globalAlpha=1;ctx.setLineDash([]);ctx.fillStyle='#e9f5f8';ctx.font='10px Inter Variable, sans-serif';ctx.fillText(legend,12,height-9,Math.max(0,width-24));ctx.restore();
}
