import {expect,it} from 'vitest';
import {journalContent,type JournalReport} from './journal';
it('distinguishes empty, unavailable and loading histories without simulated sample rows',()=>{
 expect(journalContent(undefined,true,'')).toContain('Recuperando operaciones guardadas');
 expect(journalContent(undefined,false,'<error>')).toContain('&lt;error&gt;');
 expect(journalContent(undefined,false,'<error>')).not.toContain('Todavía no hay operaciones');
 const data:JournalReport={page:{source:'RUST_SIMULATION',entries:[],nextBeforeId:null},performance:{total_trades:0,wins:0,losses:0,win_rate:0,average_pnl_points:0,average_mfe:0,average_mae:0,target_1_hit_rate:0,target_2_hit_rate:0,stop_rate:0},legacyArchivedTrades:15,entriesEnabled:false};
 expect(journalContent(data,false,'')).toContain('Todavía no hay operaciones Rust');
 data.page.entries.push({id:1,session:'2026-09-24',symbol:'<QQQ>',trade:{side:'CALL',entry:100,current:101,stop:98,target1:102,target2:null,mfe:1,mae:0,pnlPoints:1,pnlPercent:1,stage:'<INITIAL>',openedMs:1790258400000,closedMs:null,exitReason:null}});
 const html=journalContent(data,false,'');expect(html).toContain('&lt;QQQ&gt;');expect(html).toContain('&lt;INITIAL&gt;');expect(html).toContain('Al último precio observado');expect(html).toContain('15 operaciones antiguas archivadas');
});
