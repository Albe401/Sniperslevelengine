import {expect,it} from 'vitest';
import {ROUTE_BAR_MS,ROUTE_SEQUENCE_WINDOW_MS,routeSequenceDeadlineMs,routeSequenceLastAllowedBarStartMs,routeSequenceBarExpired,routeSequenceAdmissionExpired} from './route-sequence-policy';

const impulseStartMs=1793012400000;
const impulseCloseMs=impulseStartMs+ROUTE_BAR_MS;
const deadlineMs=impulseCloseMs+600000;

it('keeps the v1 calendar while measuring ten minutes from the impulse close',()=>{
 expect(ROUTE_SEQUENCE_WINDOW_MS).toBe(600000);
 expect(routeSequenceDeadlineMs(impulseCloseMs)).toBe(deadlineMs);
 expect(routeSequenceLastAllowedBarStartMs(impulseStartMs)).toBe(impulseStartMs+600000);
 expect(routeSequenceLastAllowedBarStartMs(impulseStartMs)+ROUTE_BAR_MS).toBe(deadlineMs);
});

it.each([-1,0,1])('uses an inclusive observation close and an exclusive new admission at deadline %d ms',offset=>{
 const now=deadlineMs+offset;
 expect(routeSequenceBarExpired(now,deadlineMs)).toBe(offset>0);
 expect(routeSequenceAdmissionExpired(now,deadlineMs)).toBe(offset>=0);
});

it('does not turn a causal last close into an available new entry after the deadline',()=>{
 const receivedMs=deadlineMs+125;
 expect(routeSequenceBarExpired(deadlineMs,deadlineMs)).toBe(false);
 expect(routeSequenceAdmissionExpired(receivedMs,deadlineMs)).toBe(true);
 expect(routeSequenceDeadlineMs(impulseCloseMs)).toBe(deadlineMs);
});
