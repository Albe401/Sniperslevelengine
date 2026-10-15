import {quickReadingPanels} from './quick-reading-panels';
import {buildIntradayControl} from './intraday-control';
import {bindLevelCardSpacing} from './level-card-spacing';
import {escapeHtml as esc} from './model';
import {seedCapturedAnalystTargets} from './analyst-targets';
import type {TargetRow} from './analyst-targets-cache';
import type {LiveState} from './live';
import './styles.css';import './restrained.css';import './visual-readings.css';import './brand.css';import './day-map.css';import './reading-tabs.css';import './compact-reading-height.css';
import '@fontsource-variable/inter';

export function paintDetailedSnapshot(frame:Record<string,unknown>,capturedMs:number,extras:Record<string,{command:string;observedMs:number;value:unknown}>={}){
 const symbol=String(frame.symbol);const host=document.querySelector<HTMLElement>('#snapshot-image-root')!;
 const time=new Date(capturedMs).toLocaleString('es',{timeZone:'America/New_York',hour12:false});
 let reading='';
 const targets=Object.values(extras).filter(r=>r.command==='get_fmp_targets'&&r.observedMs<=capturedMs).sort((a,b)=>b.observedMs-a.observedMs)[0];
 if(targets)seedCapturedAnalystTargets((targets.value as {rows:Record<string,TargetRow>}).rows,capturedMs);
 if(frame.snapshotUnavailable||!frame.stocks||!frame.options){reading='<section class="panel">Datos pendientes para este activo en la captura.</section><section class="panel">Mapa de niveles · AMT · mapa estructural: pendientes.</section>';}
 else {
  const s=structuredClone(frame) as unknown as LiveState;
  const active=s.stocks.phase==='streaming';
  const control=buildIntradayControl(s,symbol,active,capturedMs);
  reading=quickReadingPanels(s,symbol,active,control,capturedMs);
 }
 host.innerHTML=`<header class="snapshot-image-title"><strong>${esc(symbol)} · LECTURA DETALLADA · SNAPSHOT</strong><span>${esc(time)} ET</span></header><section id="live-workspace" data-live-view="detailed" class="visual-reading">${reading}</section>`;
 bindLevelCardSpacing();
 // This capture ends at the upper part of the chart, after both complete maps.
 const layout=host.querySelector<HTMLElement>('.quick-command-layout');
 if(layout){
  const track=layout.querySelector<HTMLElement>('.day-map-track');
  if(track){const bottom=Math.max(0,...Array.from(track.querySelectorAll<HTMLElement>(':scope > .day-map-node')).map(n=>parseFloat(n.style.top||'0')+n.offsetHeight));
   const height=Math.ceil(Math.max(layout.offsetHeight,bottom+track.getBoundingClientRect().top-layout.getBoundingClientRect().top+40));
   layout.style.setProperty('--level-overview-height',`${height}px`);
  }
 }
 const graph=layout?.nextElementSibling as HTMLElement|null;
 if(graph){graph.classList.add('snapshot-image-chart-top');graph.style.maxHeight='86px';graph.style.overflow='hidden';}
 let after=graph?.nextElementSibling;
 while(after){const next=after.nextElementSibling;after.remove();after=next;}
}
