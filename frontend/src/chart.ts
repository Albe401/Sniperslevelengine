import { aggregate, marketTime, price, escapeHtml as esc, type Asset } from './model';

export interface ChartMark {timestampMs:number;price?:number;source?:'broker';label:string;kind:'entry'|'exit'}
export interface ChartGeometry {left:number;end:number;candleEnd:number;top:number;floor:number;min:number;max:number;x:(index:number)=>number;y:(price:number)=>number}
export interface ChartOptions {
  viewport?:{start:number;count:number};
  prepared?:boolean;
  gex?:boolean;
  height?:number;
  scalePrices?:number[];
  priceRange?:[number,number];
  overlay?:(geometry:ChartGeometry)=>string;
  candleDetail?:(index:number)=>string;
}

export function chart(asset: Asset, cursor: number, minutes: number, showLevels: boolean, mode: 'demo'|'live' = 'demo', marks:ChartMark[]=[], options:ChartOptions={}): string {
  const source = asset.candles.slice(0, cursor);
  const bars = options.prepared ? source : aggregate(source, minutes);
  if(!bars.length)return '<div class="live-empty">Sin velas para este intervalo.</div>';
  const W = options.gex?1100:900, H = options.gex?(options.height??560):350, left = options.gex?24:14, right = options.gex?90:78, top = options.gex?34:22, floor = options.gex?H-130:270;
  const candleEnd=options.gex?770:W-right, volumeFloor=H-33;
  const levels = showLevels ? asset.levels.filter(l=>Number.isFinite(l.price)&&l.price>0) : [];
  // Only anchor to a candle that actually covers the event; never snap a marker
  // across a data gap or place another session's entry on the first visible bar.
  const visibleMarks=marks.filter(m=>Number.isFinite(m.timestampMs)&&(m.price===undefined||Number.isFinite(m.price)&&m.price>0))
    .map(mark=>({mark,index:bars.findIndex(b=>mark.timestampMs>=b.timestampMs&&mark.timestampMs<b.timestampMs+minutes*60000)}))
    .filter(m=>m.index>=0);
  const visibleBars=options.viewport?bars.filter((_,i)=>i>=options.viewport!.start-1&&i<=options.viewport!.start+options.viewport!.count):bars;
  const scaleBars=visibleBars.length?visibleBars:bars.slice(-1);
  const values = [...scaleBars.flatMap(c => [c.high, c.low]), ...levels.map(l => l.price),...(options.scalePrices??[]).filter(p=>Number.isFinite(p)&&p>0)];
  const markerPadding=visibleMarks.length?(Math.max(...values)-Math.min(...values))*.15:0;
  const padding=Math.max(markerPadding,options.gex?Math.max((Math.max(...values)-Math.min(...values))*.06,Math.max(...values)*.001):.4);
  const manual=options.priceRange?.every(Number.isFinite)&&options.priceRange[0]>0&&options.priceRange[1]>options.priceRange[0]?options.priceRange:undefined;
  const min = manual?.[0]??Math.min(...values) - padding, max = manual?.[1]??Math.max(...values) + padding;
  const y = (p: number) => top + (max - p) / (max - min) * (floor - top);
  const step = (candleEnd - left) / Math.max(options.viewport?.count??bars.length, 1);
  const x = (i: number) => left + (i - (options.viewport?.start??0) + 0.5) * step;
  const last = bars[bars.length - 1];
  const maxVolume = Math.max(...bars.map(c => c.volume), 1);
  const grid = Array.from({length: 6}, (_, i) => {
    const p = min + (max - min) * i / 5;
    return `<line x1="0" x2="${W-right}" y1="${y(p)}" y2="${y(p)}" class="grid-line"/><text x="${W-right+14}" y="${y(p)+4}" class="axis">${price(p)}</text>`;
  }).join('');
  const candles = bars.map((c, i) => {
    if(x(i)<left-step||x(i)>candleEnd+step)return "";
    const clamp=(p:number)=>Math.max(min,Math.min(max,p));
    const up = c.close >= c.open, color = up ? 'var(--success)' : 'var(--danger)';
    const width = Math.max(2, Math.min(step * 0.56, 13));
    return `<g><title>${marketTime(c.timestampMs)} ET · O ${price(c.open)} H ${price(c.high)} L ${price(c.low)} C ${price(c.close)} · Vol ${c.volume}${options.candleDetail?` · ${esc(options.candleDetail(i))}`:''}</title>
      ${c.high>=min&&c.low<=max?`<line x1="${x(i)}" x2="${x(i)}" y1="${y(clamp(c.high))}" y2="${y(clamp(c.low))}" stroke="${color}"/>`:""}
      ${c.high>=min&&c.low<=max?`<rect x="${x(i)-width/2}" y="${Math.min(y(clamp(c.open)), y(clamp(c.close)))}" width="${width}" height="${Math.max(1.7, Math.abs(y(clamp(c.open))-y(clamp(c.close))))}" rx="1" fill="${color}"/>`:""}
      <rect class="candle-volume" x="${x(i)-width/2}" y="${volumeFloor-c.volume/maxVolume*29}" width="${width}" height="${c.volume/maxVolume*29}" fill="${color}" opacity=".35"/></g>`;
  }).join('');
  const overlays = levels.map(l => {
    const color = l.kind === 'magnet' ? 'var(--mint)' : l.kind === 'resistance' ? 'var(--warning)' : 'var(--success)';
    return `<line x1="0" x2="${W-right}" y1="${y(l.price)}" y2="${y(l.price)}" stroke="${color}" stroke-opacity=".5" stroke-dasharray="4 5"/><text x="20" y="${y(l.price)-8}" fill="${color}" class="level-label">${esc(l.label.toUpperCase())}</text>`;
  }).join('');
  const times = bars.filter((_, i) => x(i)>=left&&x(i)<=candleEnd&&i % Math.max(1, Math.floor((options.viewport?.count??bars.length)/5)) === 0).map(c => {
    const i = bars.indexOf(c);
    return `<text x="${x(i)}" y="${H-8}" text-anchor="middle" class="axis">${marketTime(c.timestampMs)}</text>`;
  }).join('');
  const stacks=new Map<string,number>();
  const markers=visibleMarks.map(({mark:m,index})=>{
    const key=`${index}:${m.kind}`,stack=stacks.get(key)??0;stacks.set(key,stack+1);
    const entry=m.kind==='entry',bar=bars[index],cx=x(index),tip=y(entry?bar.low:bar.high)+(entry?7:-7)+(entry?1:-1)*stack*24;
    const base=tip+(entry?10:-10),labelY=tip+(entry?22:-15),color=entry?'#20f5ad':'#ffca4a';
    return `<g class="paper-chart-mark" data-mark-kind="${m.kind}" data-mark-source="${m.source??'model'}" data-mark-time="${m.timestampMs}"><title>${esc(m.label)} · ${marketTime(m.timestampMs)} ET</title><polygon points="${cx},${tip} ${cx-6},${base} ${cx+6},${base}" fill="${color}" stroke="#07132b" stroke-width="1"/><text x="${cx}" y="${labelY}" text-anchor="middle" fill="${color}" font-size="10" font-weight="700">${entry?'E':'S'}${m.source==='broker'?'':' SIM'}</text></g>`;
  }).join('');
  const extra=options.overlay?.({left,end:W-right,candleEnd,top,floor,min,max,x,y})??'';
  const interaction=options.gex?`preserveAspectRatio="none" tabindex="0" data-gex-svg data-view-start="${options.viewport?.start??0}" data-step="${step}" data-left="${left}" data-end="${W-right}" data-candle-end="${candleEnd}" data-top="${top}" data-floor="${floor}" data-times="${bars.map(c=>c.timestampMs).join(',')}" data-price-min="${min}" data-price-max="${max}"`:'';
  const crosshair=options.gex?`<line data-gex-cross-x pointer-events="none" visibility="hidden" x1="0" x2="0" y1="${top}" y2="${floor}" stroke="#d1efff80" stroke-dasharray="3 3"/><line data-gex-cross-y pointer-events="none" visibility="hidden" x1="${left}" x2="${W-right}" y1="0" y2="0" stroke="#d1efff80" stroke-dasharray="3 3"/>`:'';
  return `<svg class="market-chart" ${interaction} viewBox="0 0 ${W} ${H}" role="img" aria-label="${mode==='demo'?'Velas de demostración':'Velas recibidas desde Massive'} de ${esc(asset.symbol)}, intervalo ${minutes} minutos">
    ${grid}${extra}${overlays}<defs><clipPath id="candle-bounds"><rect x="${left}" y="${top}" width="${candleEnd-left}" height="${volumeFloor-top}"/></clipPath></defs><g clip-path="url(#candle-bounds)">${candles}${markers}</g>${crosshair}<line x1="0" x2="${W-right}" y1="${y(last.close)}" y2="${y(last.close)}" stroke="var(--mint)" stroke-opacity=".45" stroke-dasharray="2 4"/>
    <rect x="${W-right+3}" y="${y(last.close)-11}" width="72" height="22" rx="4" fill="var(--mint)"/><text x="${W-right+39}" y="${y(last.close)+4}" text-anchor="middle" class="last-price">${price(last.close)}</text>${times}
  </svg>`;
}

export function sparkline(asset: Asset, cursor: number): string {
  const c = asset.candles.slice(Math.max(0, cursor-30), cursor);
  const lo = Math.min(...c.map(b => b.close)), hi = Math.max(...c.map(b => b.close));
  return `<svg viewBox="0 0 110 30" aria-hidden="true"><polyline points="${c.map((b,i) => `${i/Math.max(1,c.length-1)*110},${27-(b.close-lo)/(hi-lo || 1)*24}`).join(' ')}" fill="none" stroke="currentColor" stroke-width="1.7"/></svg>`;
}
