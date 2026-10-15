import {expect,it} from 'vitest';
import {optionPrintCards,optionPrintPanel,type OptionPrintReport} from './option-print-scanner';

const report:OptionPrintReport={symbol:'QQQ',date:'2026-10-08',scannedMs:Date.UTC(2026,9,8,15),minContracts:50,chainExamined:500,selectedContracts:16,chainComplete:false,tradesComplete:true,matchingPrints:2,warnings:[],rows:[
 {ticker:'O:QQQ261008C00500000',side:'CALL',expiration:'2026-10-08',strike:500,executedMs:Date.UTC(2026,9,8,14,52,3),sipMs:Date.UTC(2026,9,8,14,52,3),size:120,price:1.25,premium:15000,exchange:46,sequence:8},
 {ticker:'O:QQQ261008P00490000',side:'PUT',expiration:'2026-10-08',strike:490,executedMs:Date.UTC(2026,9,8,14,51,2),sipMs:Date.UTC(2026,9,8,14,51,2),size:80,price:2,premium:16000,exchange:46,sequence:9},
]};
it('shows CALL and PUT prints with date, ET time, size, and bounded coverage',()=>{const html=optionPrintCards(report,'today');expect(html).toContain('op-card call');expect(html).toContain('op-card put');expect(html).toContain('10:52:03');expect(html).toContain('2026-10-08');expect(html).toContain('120');expect(html).toContain('Se consultan hasta 8 CALL y 8 PUT');expect(html).not.toContain('Compra verificada');});
it('keeps a previous-day control visible and never fabricates saved prints',()=>{const html=optionPrintPanel();expect(html).toContain('data-op-view="previous"');expect(html).toContain('Día anterior');expect(optionPrintCards(null,'previous')).toContain('Aún no hay una sesión anterior guardada');expect(optionPrintCards(report,'previous')).toContain('DÍA ANTERIOR GUARDADO');});
it('labels SIP-only trades without claiming a verified execution clock',()=>{
 const sip:OptionPrintReport={...report,rows:[{...report.rows[0],executedMs:null,timestampSource:'sip'}]};
 const html=optionPrintCards(sip,'today');
 expect(html).toContain('Fecha y hora de reporte SIP');expect(html).toContain('10:52:03');
 expect(html).not.toContain('Fecha y hora de ejecución');
 expect(html).toContain('no verifica la hora de ejecución');
});
