import {describe,it,expect} from 'vitest';
import {botStatus,botPage,connection,historyPanel,openPnl,type BotSession,type Check} from './paper-bot';
describe('Verified operation history',()=>{
 const filled:BotSession={symbol:'QQQ test',buy_id:'one',phase:'done',reason:'Salida ejecutada',entry_premium:2,exit_premium:2.3,entry_time:'2026-09-28T14:00:00Z',exit_time:'2026-09-28T14:05:00Z',exit_trigger:'TP <verified>'};
 it('shows broker fill times and gross result with escaped reasons',()=>{const html=historyPanel([filled]);expect(html).toContain('10:00:00');expect(html).toContain('10:05:00');expect(html).toContain('$30.00');expect(html).toContain('&lt;verified&gt;');});
 it('never counts an accepted entry as a closed operation',()=>{const pending={...filled,phase:'buy_pending',entry_time:null,exit_time:null,entry_premium:null,exit_premium:null};expect(historyPanel([pending],'closed')).toContain('Sin operaciones');expect(historyPanel([pending])).not.toContain('$30.00');});
 it('requires a fresh nonfuture mark for open PnL',()=>{const p={...filled,phase:'open',mark_bid:2.2,mark_ms:10000};expect(openPnl(p,11000)).toBeCloseTo(20);expect(openPnl(p,16000)).toBeNull();expect(openPnl(p,9000)).toBeNull();expect(openPnl(filled,11000)).toBeNull();});
});
describe('Paper bot status',()=>{
  it('labels experimental partial evidence without hiding entry blockers',()=>{
    const now=Date.now();
    const html=botStatus({armed:true,ready:true,updated_ms:now,strategy_reading:{reviewedMs:now,reading:{ready:false,eligible:false,net:0,coverage:20,paper_partial:true,warnings:['Cadena <parcial>'],blockers:['Falta VWAP']}}});
    expect(html).toContain('ACTIVO · ESPERANDO CONDICIONES');
    expect(html).toContain('PAPER EXPERIMENTAL');
    expect(html).toContain('Cadena &lt;parcial&gt;');
    expect(html).toContain('Falta VWAP');
  });
  it('never describes an accepted order as filled',()=>{const html=botStatus({armed:true,ready:true,session:{symbol:'QQQ',phase:'buy_pending',reason:'Esperando',buy_id:'snipers-a',last_order:{status:'accepted',filled_qty:'0'}}});expect(html).toContain('accepted');expect(html).toContain('buy_pending');expect(html).not.toContain('Entrada ejecutada');});
  it('escapes provider errors',()=>{expect(botStatus({armed:false,ready:true,error:'<script>alert(1)</script>'})).not.toContain('<script>');});
});

describe('Weekend Paper connectivity',()=>{
 const sample:Check={account_enabled:true,next_open:'2026-09-28T09:30:00-04:00',session_date:'2026-09-28',quote_error:'No current quote',quote_fresh:false,account_status:'ACTIVE',cash:'1000',options_level:3,market_open:false,source:'Massive',quote:null};
 it('preserves account verification without an available quote',()=>{const html=connection(sample,Date.now());expect(html).toContain('ACTIVE');expect(html).toContain('2026-09-28');expect(html).toContain('No current quote');expect(html).not.toContain('REST verificado');});
 it('does not label an expired check as current',()=>{expect(connection({...sample,quote_fresh:true},Date.now()-61000)).not.toContain('Muestra vigente');});
});

describe('Intraday Paper configuration',()=>{
 it('shows structural exits and unlimited daily entries',()=>{const html=botPage();expect(html).toContain('Salida estructural');expect(html).not.toContain('+20%');expect(html).not.toContain('−15%');expect(html).toContain('∞ <small>Entradas / día');expect(html).toContain('Vencimiento automático');expect(html).not.toContain('SEÑAL / DÍA');});
 it('keeps actual entry premium without fixed exit prices',()=>{const html=botStatus({armed:false,ready:true,session:{symbol:'QQQ',phase:'open',reason:'test',buy_id:'snipers-test',entry_premium:2,daily_entries:2,last_order:{filled_avg_price:'9'}}});expect(html).toContain('$2.00');expect(html).toContain('Salida estructural');expect(html).toContain('Riesgo máximo: toda la prima');expect(html).not.toContain('Objetivo $2.20');expect(html).not.toContain('TP $2.40');expect(html).not.toContain('Stop $1.70');expect(html).toContain('Entrada 2 · sin límite diario');});
});
it('shows current blocking reasons even while the bot is paused and hides stale diagnostics',()=>{const now=Date.now();const s={armed:false,ready:true,updated_ms:now,strategy_reading:{reviewedMs:now,reading:{ready:false,eligible:false,coverage:20,net:0,blockers:['Estructura DAY incompleta','Reloj <adelantado>']}}};expect(botStatus(s)).toContain('Estructura DAY incompleta');expect(botStatus(s)).toContain('Reloj &lt;adelantado&gt;');s.strategy_reading.reviewedMs=now-6000;expect(botStatus(s)).not.toContain('Estructura DAY incompleta');});

it('offers all four expiry modes with three days as default',()=>{const html=botPage();for(const mode of ['zero_dte','next','three_days','week'])expect(html).toContain('value="'+mode+'"');expect(html).toContain('value="three_days" selected');});

it('allows a user selected dollar budget without a fixed maximum',()=>{const html=botPage();const input=html.match(/<input id="bot-budget"[^>]+>/)?.[0]??'';expect(input).toContain('min="0.01"');expect(input).toContain('step="0.01"');expect(input).not.toContain('max=');expect(html).not.toContain('Máximo $5.000');});


describe('Unified session control',()=>{
 it('keeps one configuration inside the same control card and outside the polled summary',()=>{const html=botPage();const card=html.slice(html.indexOf('<section class="bot-control-card"'),html.indexOf('<div class="bot-reactor"'));expect(card).toContain('ESTADO DEL BOT');expect(card).toContain('PREPARAR SESIÓN PAPER');expect(card).toContain('</div><details class="bot-config"');expect((html.match(/<form data-bot-config>/g)||[])).toHaveLength(1);expect(card).toContain('data-bot-arm');});
 it('separates the polled summary from positions and diagnostics',()=>{const s={armed:false,ready:true,updated_ms:Date.now()};const summary=botStatus(s,'summary'),details=botStatus(s,'details');expect(summary).toContain('ENTRADAS PAUSADAS');expect(summary).not.toContain('bot-position-card');expect(details).toContain('bot-position-card');expect(details).not.toContain('bot-state-card');expect(botStatus(s)).toBe(summary+details);});
 it('keeps the strategy available in a collapsed disclosure without hiding expiry or budget',()=>{const html=botPage();expect(html).toContain('<details class="bot-config" open>');expect(html).toContain('<details class="bot-config-guide"><summary>Ver estrategia');expect(html).toContain('id="bot-expiry"');expect(html).toContain('id="bot-budget"');expect(html).not.toContain('<button data-bot-prepare>');});
});

it('offers a per-trade profit target starting at ten percent without a fixed upper cap',()=>{const html=botPage();const input=html.match(/<input id="bot-profit"[^>]+>/)?.[0]||'';expect(input).toContain('min="10"');expect(input).toContain('value="10"');expect(input).not.toContain('max=');expect(html).toContain('bruto y por operación');expect(botStatus({armed:false,ready:true,session:{symbol:'QQQ',buy_id:'test',phase:'open',reason:'test',profit_target_pct:25}})).toContain('25%');});
