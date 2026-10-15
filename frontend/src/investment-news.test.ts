import {describe,it,expect} from 'vitest';
import {newsToday,newsCard,newsSections,newsLanguageNotice,newsDisplayOrder,headlineSentiment,applyHeadlineReading,type RadarNews} from './investment-news';
const item:RadarNews={id:'test',ticker:'TEST',title:'<script>bad</script>',summary:'<img onerror=bad>',source:'Publisher',publishedAt:'invalid',sentiment:'negative',horizon:'intraday',url:'javascript:alert(1)'};
describe('News presentation',()=>{
 it('puts important colored news and analyst changes first, then gray news by relevance and time',()=>{
  const stories:RadarNews[]=[
   {...item,id:'gray-medium',sentiment:'neutral',importance:'medium',publishedAt:'2026-10-09T21:00:00Z'},
   {...item,id:'gray-high',sentiment:'unknown',importance:'high',publishedAt:'invalid'},
   {...item,id:'color-medium',sentiment:'mixed',importance:'medium',publishedAt:'2026-10-09T22:00:00Z'},
   {...item,id:'color-old',importance:'high',publishedAt:'2026-10-09T19:00:00Z'},
   {...item,id:'rating-new',sentiment:'unknown',eventType:'upgrade',importance:'high',publishedAt:'2026-10-09T20:00:00Z'},
   {...item,id:'color-new',sentiment:'positive',importance:'high',publishedAt:'2026-10-09T21:00:00Z'},
  ];
  expect([...stories].sort(newsDisplayOrder).map(n=>n.id)).toEqual(['color-new','rating-new','color-old','color-medium','gray-high','gray-medium']);
  expect(newsSections(stories)).toContain('las grises quedan al final');
 });
 it('keeps original articles visible and identifies untranslated content',()=>{expect(newsSections([item])).toContain('TEST');expect(newsLanguageNotice([item])).toContain('Se muestran los originales');expect(newsLanguageNotice([{...item,translated:true}])).toBe('');expect(newsLanguageNotice([])).toBe('');});
 it('identifies the local translator without replacing Spanish text',()=>{expect(newsLanguageNotice([{...item,translated:true,translationEngine:'Argos local'}])).toContain('Traducidas localmente');});
 it('escapes publisher content and rejects unsafe article links',()=>{const html=newsCard(item);expect(html).not.toContain('<script>');expect(html).not.toContain('<img onerror');expect(html).not.toContain('javascript:');expect(html).toContain('<h3>bad</h3>');expect(html).toContain('Fecha no disponible');});
 it('shows intraday only and excludes long term articles',()=>{const html=newsSections([item,{...item,id:'long',ticker:'LONG',horizon:'long'}]);expect(html).toContain('TEST');expect(html).not.toContain('LONG');expect(html).not.toContain('Noticias a largo plazo');});
 it('shows a disconnected state by default and labels examples explicitly',()=>{expect(newsSections()).toContain('Sin noticias visibles');expect(newsSections()).not.toContain('AVGO');expect(newsSections([],true)).toContain('noticias ficticias');});
});
describe('Headline colors',()=>{
 it('hides low relevance without hiding important neutral news',()=>{const html=newsSections([{...item,ticker:'NOISE',importance:'low'},{...item,ticker:'MATERIAL',sentiment:'neutral',importance:'high'}]);expect(html).not.toContain('NOISE');expect(html).toContain('MATERIAL');});
 it('shows Spanish text and safely retains the original',()=>{const html=newsCard({...item,title:'Mejora de previsiones',summary:'Resumen en español',originalTitle:'<img onerror=bad>',translated:true,eventType:'upgrade'});expect(html).toContain('Mejora de previsiones');expect(html).toContain('Ver original');expect(html).toContain('Mejora de analista');expect(html).not.toContain('<img onerror');});
 it('recognizes explicit cues and conflicting cues',()=>{expect(headlineSentiment('Company raises guidance')).toBe('positive');expect(headlineSentiment('Analyst downgrades Company')).toBe('negative');expect(headlineSentiment('Revenue beats estimates but company cuts guidance')).toBe('mixed');});
 it('does not classify hypothetical or negated headlines',()=>{for(const title of ['Company may raise guidance','Company not upgraded','Company announces event'])expect(headlineSentiment(title)).toBe('unknown');});
 it('labels the local reading separately',()=>{expect(applyHeadlineReading({...item,title:'Company raises guidance',sentiment:'unknown'}).sentimentBasis).toBe('Lectura automática del titular');expect(applyHeadlineReading(item)).toBe(item);});
});

it('separates Saturday news from Friday using New York dates and rejects future dates',()=>{const now=Date.parse('2026-10-10T15:30:00Z');const news=(id:string,publishedAt:string)=>({...item,id,publishedAt});expect(newsToday([news('friday','2026-10-10T03:59:00Z'),news('saturday','2026-10-10T04:00:00Z'),news('future','2026-10-11T04:00:00Z'),news('invalid','bad')],now).map(n=>n.id)).toEqual(['saturday']);});
