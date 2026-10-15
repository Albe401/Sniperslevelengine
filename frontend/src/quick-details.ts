import {topStrikes} from './top-strikes';
import type {LiveState} from './live';
import {escapeHtml as esc,price} from './model';
import {interpretationFresh} from './confirmations';
import {rangeFresh} from './daily-range';
import {rangeContextFresh} from './range-context';
import {displayStructure,currentMapHorizon,offSession} from './reading-display';
import './quick-details.css';

const num=(n:number|null|undefined)=>n!=null&&Number.isFinite(n)?price(n):'—';
const magnetStrength=(score:number|null|undefined)=>score!=null&&Number.isFinite(score)&&score>=0&&score<=100?`Fuerza ${num(score)}/100`:'Fuerza pendiente';
const count=(n:number|null|undefined)=>n!=null&&Number.isFinite(n)?Intl.NumberFormat('es').format(n):'—';
const cell=(label:string,value:string,tone='cyan',detail='')=>`<div class="qd-cell ${tone}"><span>${esc(label)}</span><strong>${esc(value)}</strong>${detail?`<small>${esc(detail)}</small>`:''}</div>`;

export function quickDetails(state:LiveState|undefined,symbol:string,active:boolean,now=Date.now()):string {
 const connected=active&&state?.symbol===symbol&&interpretationFresh(state?.publishedMs,now);
 const snapshot=state?.optionSnapshotStatus,h=currentMapHorizon(snapshot,now);
 const fresh=!!(connected&&snapshot?.phase==='ready'&&rangeFresh(snapshot,now));
 const s=displayStructure(h,fresh),partial=!!(s&&h?.structure.provisional);
 const recent=(ms:number|null|undefined)=>ms!=null&&ms<=now&&now-ms<=15000;
 const live=connected&&state?.stocks.phase==='streaming'&&recent(state?.ticks[symbol]?.tradeMs);
 const optionsLive=connected&&state?.options.phase==='streaming'&&recent(state?.options.lastEventMs);
 const pressure=fresh&&!partial?h?.structure.quickPressure:null;
 const scenarios=['CALL','PUT'].map(side=>{
  const call=side==='CALL',support=call?s?.putWall:s?.callWall,route=s?.routes.find(r=>r.direction===side);
  const confirmation=state?.wallPull?.[`${h?.mode} ${side}`];
  const verified=live&&optionsLive&&h?.mode==='DAY'&&!partial&&fresh;
  const sidePressure=pressure==null?null:call?pressure:100-pressure;
  return `<article class="qd-panel qd-scenario ${call?'green':'red'}"><header><h3>Escenario ${side}</h3><span class="qd-status">${verified&&confirmation?.ready?'ALINEADO':'ESPERAR'}</span></header><div class="qd-grid">${cell(call?'Soporte · PUT WALL':'Soporte · CALL WALL',num(support?.strike),call?'green':'red',`Fuerza ${num(support?.score)}/100`)}${cell(`${side} PULL`,num(route?.pullTarget),call?'green':'red',`Puntuación ${num(route?.score)}/100`)}${cell('Volumen zona · snapshot',count(support?.zoneVolume),'white',`OI ${count(support?.zoneOi)}`)}${cell('Barrera',num(route?.barrier),'yellow',`${route?.pathState??'Sin recorrido'} · ${num(route?.barrierScore)}/100`)}${cell('Camino · liquidez',num(route?.pathLiquidity),'yellow',`Apertura ${num(route?.pathOpenness)}/100`)}${cell('Presión estructural',sidePressure==null?'—':`${num(sidePressure)} %`,call?'green':'red')}${cell('Volumen live en zonas',verified&&confirmation?.liveReady?`${confirmation.liveWindowUsed} MIN · CUBIERTO`:'PENDIENTE','yellow')}</div><p>${esc(partial?'Referencias provisionales: cadena incompleta.':verified?confirmation?.reason??'Esperando confirmación del modelo.':'Esperando evidencia intradía vigente; sin confirmación de entrada.')}</p></article>`;
 }).join('');
 const r=state?.rangeContext,rangeLive=rangeContextFresh(r,symbol,!!connected,state?.publishedMs,now);
 const model=fresh&&snapshot?.dailyRange?.ready?snapshot.dailyRange:undefined;
 const lower=rangeLive?r?.lower:model?.snapshotLower,upper=rangeLive?r?.upper:model?.snapshotUpper;
 const center=rangeLive?r?.anchor:model?.spot;
 const range=`<article class="qd-panel qd-range"><header><h3>Rango esperado del día</h3><span class="qd-status">${rangeLive?'ANCLA RTH':model?'SNAPSHOT':'PENDIENTE'}</span></header><div class="qd-band">${cell('Inferior',num(lower),'cyan')}<span>←</span>${cell(rangeLive?'Apertura 09:30 ET':'Referencia',num(center),'white')}<span>→</span>${cell('Superior',num(upper),'cyan')}</div><div class="qd-grid">${cell('Movimiento esperado',model?`± ${num(model.expectedMoveDollars)}`:'—','yellow',model?`${num(model.expectedMovePct)} %`:'')}${cell('IV ATM',model?.atmIv==null?'—':`${num(model.atmIv*100)} %`,'cyan',`Exp. ${model?.nearestExpiration??'—'}`)}${cell('Restante CALL',num(rangeLive?r?.remainingCall:null),'green')}${cell('Restante PUT',num(rangeLive?r?.remainingPut:null),'red')}</div><p>${esc(rangeLive?r?.rangeState??'':model?'Referencia del snapshot; esperando apertura RTH.':'Esperando estimación del motor.')} · El rango no confirma entradas.</p></article>`;
 const off=connected?state?.offExchange:undefined,session=offSession(off?.analytics.session,off?.analytics.history);
 const historical=session?.date!==off?.analytics.session?.date;
 const level=session?.top_levels[0],confirmation=connected&&h?state?.magnetConfirmations?.[h.mode]:undefined;
 const certified=live&&fresh&&!partial&&off?.analytics.live_fresh&&['CONFIRMED','CONFIRMED_STRONG','PARTIAL_CONFIRMATION','WEAK_CONFIRMATION','DIVERGENCIA'].includes(confirmation?.status??'');
 const confirmationLabels:Record<string,string>={CONFIRMED:'CONFIRMADO',CONFIRMED_STRONG:'CONFIRMACIÓN FUERTE',PARTIAL_CONFIRMATION:'CONFIRMACIÓN PARCIAL',WEAK_CONFIRMATION:'CONFIRMACIÓN DÉBIL',DIVERGENCIA:'DIVERGENCIA'};
 const up=s?.routes.find(r=>r.direction==='CALL'),down=s?.routes.find(r=>r.direction==='PUT');
 const zone=(r:typeof up)=>r?.vacuumLow==null||r.vacuumHigh==null?'—':`${num(r.vacuumLow)} – ${num(r.vacuumHigh)}`;
 const targets=[cell('CALL WALL',num(s?.callWall?.strike),'green',`Fuerza ${num(s?.callWall?.score)}/100`),cell('PUT WALL',num(s?.putWall?.strike),'red',`Fuerza ${num(s?.putWall?.score)}/100`),cell('Imán de liquidez',num(s?.magnet),'yellow',magnetStrength(s?.liquidityMagnetScore)),cell('Imán principal',num(s?.selected.M1),'cyan',magnetStrength(s?.mainMagnetScore)),cell('Objetivo superior 1',num(up?.target),'green'),cell('Objetivo superior 2',num(up?.extended),'green'),cell('Objetivo inferior 1',num(down?.target),'red'),cell('Objetivo inferior 2',num(down?.extended),'red'),cell('Vacío superior',zone(up),'yellow',`Fuerza ${num(up?.vacuumScore)}/100`),cell('Vacío inferior',zone(down),'yellow',`Fuerza ${num(down?.vacuumScore)}/100`)].join('');
 const historyStatus=session?off?.history?.[session.date]:undefined;
 return `<section class="quick-details" aria-label="Escenarios y liquidez intradía"><div class="qd-heading"><h2>Escenarios · rango · liquidez</h2><span>${esc(symbol)} · ${partial?'REFERENCIAS PROVISIONALES':fresh?'SNAPSHOT DISPONIBLE':'ESPERANDO DATOS'}${fresh&&h?` · ${esc(h.expirations.join(' / '))}`:''}</span></div><div class="qd-top">${scenarios}${range}</div><div class="qd-bottom"><article class="qd-panel qd-magnet"><header><h3>Imán del día + dark pool</h3></header><div class="qd-grid">${cell('Opciones · M1',num(s?.selected.M1),'yellow',magnetStrength(s?.mainMagnetScore))}${cell(historical?'TRF · histórico':'TRF · sesión',num(level?.level),'cyan',session?.date??'Sin sesión')}</div><div class="qd-confirmation ${certified&&confirmation?.status!=='DIVERGENCIA'?'green':'yellow'}">${certified?esc(confirmationLabels[confirmation!.status]):'SIN CONFIRMACIÓN DARK POOL'}</div><div class="qd-grid">${cell('Puntuación confirmación',certified?`${num(confirmation?.confirmationScore)}/100`:'—','yellow')}${cell('Fuerza nivel TRF',level?`${num(level.strength_score)}/100`:'—','cyan',level?.strength??'Pendiente')}</div><p>${historical?'Histórico separado; no confirma el movimiento actual.':'Operaciones fuera de bolsa: ATS y no ATS.'}</p></article><article class="qd-panel qd-targets"><header><h3>Objetivos de liquidez</h3><span class="qd-status">${partial?'PROVISIONALES':'CONTEXTO'}</span></header><div class="qd-target-grid">${targets}</div></article></div><div class="qd-telemetry">${cell('Historial TRF',session?.date??'PENDIENTE','cyan',historyStatus?`${historyStatus.phase==='ready'?'Descarga completa':historyStatus.phase} · ${historyStatus.pages} páginas`:'Esperando descarga')}${cell('Conexión de mercado',live?'PRECIO EN VIVO':connected?'ESPERANDO EVENTOS':'SIN CONEXIÓN','cyan')}${cell('Operaciones de la sesión',count(session?.print_count),'yellow',`${historical?'Histórico': 'Sesión'} · ${count(session?.live_count)} recibidas en vivo`)}${cell('Cobertura del mapa',partial?`${h!.structure.provisional!.coveredContracts}/${h!.structure.provisional!.totalContracts}`:fresh&&s?'COMPLETA':'PENDIENTE','yellow',partial?'Contratos utilizados; no es una señal':'OI / volumen del snapshot')}</div>${topStrikes(h?.structure.topContracts,fresh)}</section>`;
}
