"use client";
import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import { DICTS, LANGS, type DictKey, type Lang } from "./dict";

type Params = Record<string, string | number>;
interface I18n { lang: Lang; setLang: (l: Lang) => void; t: (k: DictKey, p?: Params) => string; }

const Ctx = createContext<I18n | null>(null);
const STORE_KEY = "nt-lang";

/** Fill {name} placeholders. A value "@key" is itself translated (used by stored alerts). */
export function translate(lang: Lang, k: DictKey, p?: Params): string {
  let s = DICTS[lang][k] ?? DICTS.en[k] ?? k;
  if (p) s = s.replace(/\{(\w+)\}/g, (m, name: string) => {
    const v = p[name];
    if (v === undefined) return m;
    const str = String(v);
    return str.startsWith("@") ? translate(lang, str.slice(1) as DictKey) : str;
  });
  return s;
}

export function I18nProvider({ children }: { children: React.ReactNode }) {
  const [lang, setLangState] = useState<Lang>("es");

  // Initial language: ?lang= / #es #en, then the viewer's saved choice, else Spanish.
  useEffect(() => {
    let l: string | null = null;
    try {
      const u = new URL(window.location.href);
      l = u.searchParams.get("lang") || u.hash.replace("#", "") || null;
      if (!l || !LANGS.includes(l as Lang)) l = localStorage.getItem(STORE_KEY);
    } catch { /* storage blocked */ }
    if (l && LANGS.includes(l as Lang)) setLangState(l as Lang);
  }, []);

  useEffect(() => { document.documentElement.lang = lang; }, [lang]);

  const setLang = useCallback((l: Lang) => {
    setLangState(l);
    try { localStorage.setItem(STORE_KEY, l); } catch { /* ignore */ }
  }, []);

  const t = useCallback((k: DictKey, p?: Params) => translate(lang, k, p), [lang]);
  const value = useMemo(() => ({ lang, setLang, t }), [lang, setLang, t]);
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useI18n(): I18n {
  const c = useContext(Ctx);
  if (!c) throw new Error("useI18n must be used inside <I18nProvider>");
  return c;
}
