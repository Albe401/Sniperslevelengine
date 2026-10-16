import {describe,it,expect} from 'vitest';
import {webAutoRefreshAllowed,webRefreshLabel} from './web-refresh-policy';
describe('web automatic refresh weekdays in New York',()=>{
 for(const [iso,allowed] of [
  ['2026-10-16T12:00:00Z',true],
  ['2026-10-17T03:59:59Z',true],
  ['2026-10-17T04:00:00Z',false],
  ['2026-10-18T12:00:00Z',false],
  ['2026-10-19T03:59:59Z',false],
  ['2026-10-19T04:00:00Z',true],
  ['2026-11-07T05:00:00Z',false],
  ['2026-11-09T04:59:59Z',false],
  ['2026-11-09T05:00:00Z',true]
 ] as const)it(`${iso}: ${allowed}`,()=>expect(webAutoRefreshAllowed(Date.parse(iso))).toBe(allowed));
 it('reports the pause and rejects invalid clocks',()=>{expect(webAutoRefreshAllowed(NaN)).toBe(false);expect(webRefreshLabel(Date.parse('2026-10-18T12:00:00Z'))).toContain('pausada');});
});
