import {readFileSync} from 'node:fs';
import {it,expect} from 'vitest';
import {newsEventKey} from './news-pipeline';
import type {RadarNews} from './investment-news';
it('matches independent content grouping across captured real alternating revisions',()=>{
 const rows=JSON.parse(readFileSync(new URL('../../../docs/audits/2026-10-13-news-alert-repeat/real-revisions.json',import.meta.url),'utf8')) as {id:string;revision:number;original:[string,string,string[],string];payload:RadarNews}[];
 expect(rows.length).toBeGreaterThan(300);
 const observed=new Set<string>(),independent=new Set<string>();let changes=0;
 for(const row of rows){const n=row.payload;const expected=JSON.stringify([n.source,n.publishedAt.slice(0,10),row.original[0],row.original[1]]);const actual=newsEventKey(n);expect(actual).toBe(expected);if(!observed.has(actual))changes++;observed.add(actual);independent.add(expected);}
 expect(changes).toBe(independent.size);expect(changes).toBeLessThan(rows.length/10);
});
