import {expect,it} from 'vitest';
import {topStrikes,type ContractRanking} from './top-strikes';
const row={ticker:'O:QQQ',expiration:'2026-09-28',strike:745,score:47.1,oi:2870,volume:115247};
const ranking:ContractRanking={covered:10,total:12,calls:Array.from({length:7},(_,i)=>({...row,strike:745+i})),puts:[{...row,strike:730}]};
it('centres separate compact top-five tables with expiration and coverage',()=>{const html=topStrikes(ranking,true);expect(html).toContain('TOP 5 STRIKES CALL');expect(html).toContain('TOP 5 STRIKES PUT');expect(html).toContain('47.1');expect(html).toContain('2026-09-28');expect(html).toContain('Provisional');expect(html).not.toContain('750.00');expect(html).toContain('730.00');});
it('does not publish stale or fabricated rows',()=>{expect(topStrikes(ranking,false)).not.toContain('745.00');expect(topStrikes(undefined,true)).toContain('Esperando contratos');});
