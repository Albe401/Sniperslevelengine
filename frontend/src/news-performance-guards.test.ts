import {it,expect} from 'vitest';
import {newsPageSlice,NEWS_PAGE_SIZE} from './investment-news';
import {unseenRevisions,newsEventKey} from './news-pipeline';
import type {RadarNews} from './investment-news';
it('bounded pages preserve every article exactly once without clipping the source',()=>{const items=Array.from({length:590},(_,i)=>i);const all=Array.from({length:Math.ceil(items.length/NEWS_PAGE_SIZE)},(_,p)=>newsPageSlice(items,p).items).flat();expect(all).toEqual(items);expect(newsPageSlice(items,100).items).toHaveLength(30);expect(newsPageSlice([],0).pages).toBe(1);expect(newsPageSlice(items,NaN).index).toBe(0);});
it('linear revision handling agrees with the previous selection rule and preserves seen state',()=>{const rows=[{id:'a',revision:1},{id:'b',revision:2},{id:'c',revision:3},{id:'d',revision:1}] as RadarNews[];const seen=new Set(['a','b','c','old']),events=new Set([newsEventKey(rows[2])]);const expected=rows.filter(n=>!new Set([...seen].filter(id=>!rows.some(n=>n.id===id&&(n.revision??1)>1&&!events.has(newsEventKey(n))))).has(n.id));expect(unseenRevisions(rows,seen,events)).toEqual(expected);expect([...seen]).toEqual(['a','b','c','old']);});
