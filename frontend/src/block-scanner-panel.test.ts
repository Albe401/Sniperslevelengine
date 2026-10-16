import {describe,it,expect} from 'vitest';
import {blockScannerPanel,type CapturedBlockScanner} from './block-scanner-panel';
const now=Date.parse('2026-10-16T12:00:00Z');
function fixture():CapturedBlockScanner{return {automatic:true,enabled:true,observedMs:now,lastScanMs:now-1000,scanned:12,total:186,lastSymbol:'NVDA',error:'',morningWindow:true,config:{minimumNotional:10000000,scope:'watchlist',alertBuy:true,alertSell:true},history:[{receivedMs:now,read:false,alert:{key:'a',symbol:'NVDA',side:'BUY',executionMs:now-2000,notional:12000000,price:200,shares:60000,nbbo:'ASK'}},{receivedMs:now,read:false,alert:{key:'b',symbol:'QQQ',side:'SELL',executionMs:now-3000,notional:15000000,price:750,shares:20000,nbbo:'BID'}}]};}
describe('captured automatic block scanner',()=>{
 it('shows coverage, timestamp, threshold and both sides',()=>{const html=blockScannerPanel(fixture(),now,now);expect(html).toContain('12/186');expect(html).toContain('10,000,000');expect(html).toContain('COMPRA ESTIMADA');expect(html).toContain('VENTA ESTIMADA');expect(html).toContain('Snipers abierto');});
 it('filters the existing saved results without changing scanner scope',()=>{const html=blockScannerPanel(fixture(),now,now,'NVDA','BUY');expect(html).toContain('COMPRA ESTIMADA');expect(html).not.toContain('VENTA ESTIMADA');});
 it('never shows later events, unclassified sides or invalid notionals',()=>{const s=fixture();s.history[0].alert.executionMs=now+1;s.history[1].alert.side='UNKNOWN';expect(blockScannerPanel(s,now,now)).not.toContain('<h3>');});
 it('distinguishes missing and future states from an empty result',()=>{expect(blockScannerPanel(undefined,now,now)).toContain('pendiente');const s=fixture();s.observedMs=now+1;expect(blockScannerPanel(s,now,now)).toContain('pendiente');});
 it('marks stale snapshots and preserves history dates',()=>{const html=blockScannerPanel(fixture(),now,now+600001);expect(html).toContain('estado actual pendiente');expect(html).toContain('sesiones anteriores');});
 it('escapes untrusted error and NBBO text',()=>{const s=fixture();s.error='<script>';s.history[0].alert.nbbo='<img>';const html=blockScannerPanel(s,now,now);expect(html).toContain('&lt;script&gt;');expect(html).toContain('&lt;img&gt;');expect(html).not.toContain('<script>');});
});
