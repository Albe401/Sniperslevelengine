import {it,expect,vi} from 'vitest';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:()=>false}));
import {ResearchWatchlistSync} from './research-watchlist';
it('coalesces changes while an earlier save is still in flight',async()=>{
 let release:()=>void=()=>{};const calls:string[][]=[];
 const sync=new ResearchWatchlistSync(async symbols=>{calls.push(symbols);if(calls.length===1)await new Promise<void>(resolve=>{release=resolve;});});
 const first=sync.request(['QQQ','SPY']);sync.request(['QQQ','NVDA']);sync.request(['QQQ','AAPL']);release();await first;
 expect(calls).toEqual([['QQQ','SPY'],['QQQ','AAPL']]);await sync.request(['QQQ','AAPL']);expect(calls).toHaveLength(2);
});
it('retries failed saves and never marks an unsaved selection as committed',async()=>{
 const send=vi.fn().mockRejectedValueOnce(new Error('disk unavailable')).mockResolvedValue(undefined);const sync=new ResearchWatchlistSync(send);
 await expect(sync.request(['QQQ'])).rejects.toThrow('disk unavailable');await sync.request(['QQQ']);expect(send).toHaveBeenCalledTimes(2);
});
it('copies inputs so later changes cannot alter the in-flight selection',async()=>{
 const send=vi.fn(async()=>{}),sync=new ResearchWatchlistSync(send),symbols=['QQQ','SPY'];const p=sync.request(symbols);symbols[0]='NVDA';await p;expect(send).toHaveBeenCalledWith(['QQQ','SPY']);
});
