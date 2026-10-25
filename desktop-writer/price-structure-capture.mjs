// ../web-six-cards/desktop-writer/price-structure-capture.ts
import { readFileSync, statSync } from "node:fs";
import { createHash } from "node:crypto";

// ../web-six-cards/frontend/src/structural-technical-cache.ts
var cache = /* @__PURE__ */ new Map();
var hits = 0;
var appends = 0;
var resets = 0;
var processedBars = 0;
var initial = () => ({ count: 0, sum9: 0, sum20: 0, sum21: 0, tr: 0, plus: 0, minus: 0, dxSum: 0, dxCount: 0, tail: [], previous: null, ema9: null, ema20: null, ema21: null, ema21ThreeAgo: null, plusDi: null, minusDi: null, adx: null });
var same = (a, b) => a.timestampMs === b.timestampMs && a.endMs === b.endMs && a.open === b.open && a.high === b.high && a.low === b.low && a.close === b.close;
function advance(s, b) {
  const index = s.count;
  for (const period of [9, 20, 21]) {
    const sum = "sum" + period, ema = "ema" + period;
    if (index < period) {
      s[sum] += b.close;
      if (index === period - 1) s[ema] = s[sum] / period;
    } else {
      s[ema] += 2 / (period + 1) * (b.close - s[ema]);
    }
  }
  if (s.ema21 !== null) {
    s.tail.push(s.ema21);
    if (s.tail.length > 4) s.tail.shift();
  }
  s.ema21ThreeAgo = s.tail.length === 4 ? s.tail[0] : null;
  if (s.previous) {
    const p = s.previous, u = b.high - p.high, d = p.low - b.low, t = Math.max(b.high - b.low, Math.abs(b.high - p.close), Math.abs(b.low - p.close)), up = u > d && u > 0 ? u : 0, down = d > u && d > 0 ? d : 0;
    if (index <= 14) {
      s.tr += t;
      s.plus += up;
      s.minus += down;
    } else {
      s.tr = s.tr - s.tr / 14 + t;
      s.plus = s.plus - s.plus / 14 + up;
      s.minus = s.minus - s.minus / 14 + down;
    }
    if (index >= 14) {
      s.plusDi = s.tr > 0 ? 100 * s.plus / s.tr : 0;
      s.minusDi = s.tr > 0 ? 100 * s.minus / s.tr : 0;
      const dx = s.plusDi + s.minusDi > 0 ? 100 * Math.abs(s.plusDi - s.minusDi) / (s.plusDi + s.minusDi) : 0;
      if (s.dxCount < 14) {
        s.dxSum += dx;
        s.dxCount++;
        if (s.dxCount === 14) s.adx = s.dxSum / 14;
      } else {
        s.adx = (s.adx * 13 + dx) / 14;
      }
    }
  }
  s.previous = { ...b };
  s.count++;
  processedBars++;
}
function cachedTechnical(bars, key) {
  let record = cache.get(key), prefix = !!record && bars.length >= record.bars.length && record.bars.every((b, i) => same(b, bars[i]));
  if (!prefix) {
    record = { bars: [], state: initial() };
    resets++;
  } else if (record.bars.length === bars.length) {
    hits++;
  } else {
    appends++;
  }
  for (let i = record.bars.length; i < bars.length; i++) {
    advance(record.state, bars[i]);
    record.bars.push({ ...bars[i] });
  }
  cache.delete(key);
  cache.set(key, record);
  while (cache.size > 10) cache.delete(cache.keys().next().value);
  const s = record.state;
  return { ema9: s.ema9, ema20: s.ema20, ema21: s.ema21, ema21ThreeAgo: s.ema21ThreeAgo, plusDi: s.plusDi, minusDi: s.minusDi, adx: s.adx };
}

// ../web-six-cards/frontend/src/structural-indicators.ts
var STRUCTURAL_INDICATOR_VERSION = "structural-indicators-v1";
var finite = (x) => typeof x === "number" && Number.isFinite(x);
var timeET = new Intl.DateTimeFormat("en-US", { timeZone: "America/New_York", hour: "2-digit", minute: "2-digit", hourCycle: "h23" });
function emaSeries(bars, period) {
  const result2 = Array(bars.length).fill(NaN);
  if (!Number.isInteger(period) || period < 1 || bars.length < period) return result2;
  let e = bars.slice(0, period).reduce((n, b) => n + b.close, 0) / period;
  result2[period - 1] = e;
  const alpha = 2 / (period + 1);
  for (let i = period; i < bars.length; i++) {
    e += alpha * (bars[i].close - e);
    result2[i] = e;
  }
  return result2;
}
function dmi14(bars) {
  let tr = 0, plus = 0, minus = 0, adx = null;
  const dxs = [];
  let plusDi = null, minusDi = null;
  for (let i = 1; i < bars.length; i++) {
    const b = bars[i], p = bars[i - 1], u = b.high - p.high, d = p.low - b.low;
    const t = Math.max(b.high - b.low, Math.abs(b.high - p.close), Math.abs(b.low - p.close));
    const up = u > d && u > 0 ? u : 0, down = d > u && d > 0 ? d : 0;
    if (i <= 14) {
      tr += t;
      plus += up;
      minus += down;
    } else {
      tr = tr - tr / 14 + t;
      plus = plus - plus / 14 + up;
      minus = minus - minus / 14 + down;
    }
    if (i < 14) continue;
    plusDi = tr > 0 ? 100 * plus / tr : 0;
    minusDi = tr > 0 ? 100 * minus / tr : 0;
    const dx = plusDi + minusDi > 0 ? 100 * Math.abs(plusDi - minusDi) / (plusDi + minusDi) : 0;
    if (dxs.length < 14) {
      dxs.push(dx);
      if (dxs.length === 14) adx = dxs.reduce((n, v) => n + v, 0) / 14;
    } else adx = (adx * 13 + dx) / 14;
  }
  return { plusDi, minusDi, adx };
}
function comparableRvol(bars, interval) {
  const last = bars.at(-1), method = interval < 1440 ? "20 sesiones previas, misma hora ET y duraci\xF3n RTH" : interval === 1440 ? "20 sesiones previas de igual duraci\xF3n RTH" : "20 semanas previas de igual duraci\xF3n burs\xE1til";
  const result2 = { value: null, references: 0, required: 20, mean: null, method };
  if (!last) return result2;
  const duration = (b) => b.tradingMinutes ?? (b.endMs - b.timestampMs) / 6e4;
  const matches = [];
  const offset = interval < 1440 ? timeET.format(last.timestampMs) : "";
  for (let j = bars.length - 2; j >= 0 && matches.length < 20; j--) {
    const b = bars[j];
    if (duration(b) === duration(last) && (interval >= 1440 || b.sessionDate !== last.sessionDate && timeET.format(b.timestampMs) === offset)) matches.push(b);
  }
  result2.references = matches.length;
  if (matches.length !== 20) return result2;
  result2.mean = matches.reduce((n, b) => n + b.volume, 0) / 20;
  if (result2.mean > 0 && finite(last.volume) && last.volume >= 0) result2.value = last.volume / result2.mean;
  return result2;
}
function structuralIndicators(bars, interval, atr, cacheKey) {
  const primary = cacheKey ? cachedTechnical(bars, cacheKey) : void 0;
  const last = bars.at(-1), e21 = primary ? [] : emaSeries(bars, 21), e9 = primary ? [] : emaSeries(bars, 9), e20 = primary ? [] : emaSeries(bars, 20), di = primary ? { plusDi: primary.plusDi, minusDi: primary.minusDi, adx: primary.adx } : dmi14(bars);
  const number = (x) => finite(x) ? x : null, ema21 = primary ? primary.ema21 : number(e21.at(-1));
  const alt = bars.slice(14), alternative = cacheKey ? cachedTechnical(alt, cacheKey + ":seed14") : void 0, altE = alternative ? alternative.ema21 : emaSeries(alt, 21).at(-1), altD = alternative ? { adx: alternative.adx } : dmi14(alt);
  const emaSeedAtr = atr != null && atr > 0 && ema21 != null && finite(altE) ? Math.abs(altE - ema21) / atr : null;
  const adxSeedPoints = di.adx != null && altD.adx != null ? Math.abs(di.adx - altD.adx) : null;
  const ready = bars.length >= 100 && atr != null && atr > 0 && emaSeedAtr != null && emaSeedAtr <= 0.01 && adxSeedPoints != null && adxSeedPoints <= 0.5;
  const distanceAtr = last && ema21 != null && atr != null && atr > 0 ? (last.close - ema21) / atr : null;
  const prior = primary ? primary.ema21ThreeAgo : e21.at(-4), slope3Atr = ema21 != null && finite(prior) && atr != null && atr > 0 ? (ema21 - prior) / atr : null;
  const trend = !ready ? "PENDIENTE" : distanceAtr > 0 && slope3Atr > 0 && di.plusDi > di.minusDi ? "CALL" : distanceAtr < 0 && slope3Atr < 0 && di.plusDi < di.minusDi ? "PUT" : "MIXTA";
  return { version: STRUCTURAL_INDICATOR_VERSION, at: last?.endMs ?? 0, bars: bars.length, ready, reason: ready ? "Indicadores de las mismas velas cerradas." : "Esperando 100 velas y estabilidad de EMA/ADX.", ema9: primary ? primary.ema9 : number(e9.at(-1)), ema20: primary ? primary.ema20 : number(e20.at(-1)), ema21, distanceAtr, slope3Atr, ...di, emaSeedAtr, adxSeedPoints, trend, strength: !ready ? "PENDIENTE" : di.adx >= 25 ? "FUERTE" : di.adx >= 20 ? "TRANSICI\xD3N" : "D\xC9BIL", rvol: comparableRvol(bars, interval) };
}

// ../web-six-cards/frontend/src/price-structure-long.ts
var day = new Intl.DateTimeFormat("en-CA", { timeZone: "America/New_York" });
var finite2 = (n) => typeof n === "number" && Number.isFinite(n);
var week = (date2) => {
  const d = /* @__PURE__ */ new Date(date2 + "T12:00:00Z");
  d.setUTCDate(d.getUTCDate() - (d.getUTCDay() + 6) % 7);
  return d.toISOString().slice(0, 10);
};
function longStructureBars(h, symbol, interval, now) {
  const fail = (reason) => ({ bars: [], gaps: 0, reason });
  if (!h) return fail("Esperando historial diario multianual.");
  if (h.symbol !== symbol || !finite2(h.asOfMs) || h.asOfMs > now || day.format(h.asOfMs) !== day.format(now) || !finite2(h.throughMs) || h.throughMs > h.asOfMs || !Array.isArray(h.sessions) || !Array.isArray(h.days) || h.sessions.length > 1500 || h.days.length > 1500) return fail("Historial diario ajeno, antiguo o inv\xE1lido.");
  const sessions = h.sessions;
  if (!sessions.length || sessions.some((s, i) => !/^\d{4}-\d{2}-\d{2}$/.test(s.date) || ![s.openMs, s.closeMs].every(finite2) || s.openMs % 6e4 !== 0 || s.closeMs % 6e4 !== 0 || s.closeMs <= s.openMs || s.closeMs - s.openMs > 864e5 || day.format(s.openMs) !== s.date || i > 0 && (s.openMs <= sessions[i - 1].closeMs || s.date <= sessions[i - 1].date))) return fail("Calendario diario inv\xE1lido.");
  const latest = sessions.filter((s) => s.closeMs <= now).at(-1);
  if (latest?.closeMs !== h.throughMs) return fail("Historial diario pendiente del \xFAltimo cierre.");
  const calendar = new Map(sessions.map((s) => [s.date, s])), rows = /* @__PURE__ */ new Map();
  for (const b of h.days) {
    const s = calendar.get(b.date);
    if (!s || s.closeMs > now || rows.has(b.date) || ![b.open, b.high, b.low, b.close, b.volume].every(finite2) || Math.min(b.open, b.low, b.close) <= 0 || b.high < Math.max(b.open, b.low, b.close) || b.low > Math.min(b.open, b.close) || b.volume < 0) return fail("Velas diarias inv\xE1lidas, duplicadas o abiertas.");
    rows.set(b.date, { ...b, timestampMs: s.openMs, endMs: s.closeMs, sessionDate: s.date, tradingMinutes: (s.closeMs - s.openMs) / 6e4 });
  }
  const first = [...rows.keys()].sort()[0];
  if (!first) return fail("Sin velas diarias cerradas.");
  let bars = [], gaps = 0;
  if (interval === 1440) {
    for (const s of sessions) {
      if (s.closeMs > now || s.date < first) continue;
      const b = rows.get(s.date);
      if (!b) {
        bars = [];
        gaps++;
      } else bars.push(b);
    }
  } else {
    const groups = /* @__PURE__ */ new Map();
    for (const s of sessions) {
      const key = week(s.date);
      groups.set(key, [...groups.get(key) ?? [], s]);
    }
    for (const group of groups.values()) {
      if (group.at(-1).closeMs > now || group[0].date < first) continue;
      const complete = group.map((s) => rows.get(s.date));
      if (complete.some((b) => !b)) {
        bars = [];
        gaps++;
        continue;
      }
      const bs = complete, last = bs.at(-1);
      bars.push({ timestampMs: bs[0].timestampMs, endMs: last.endMs, sessionDate: last.sessionDate, open: bs[0].open, high: Math.max(...bs.map((b) => b.high)), low: Math.min(...bs.map((b) => b.low)), close: last.close, volume: bs.reduce((n, b) => n + b.volume, 0), tradingMinutes: bs.reduce((n, b) => n + (b.tradingMinutes ?? 0), 0) });
    }
  }
  return { bars, gaps, reason: bars.length ? "" : gaps ? "Faltan sesiones en la \xFAltima secuencia." : "Sin semanas completas cerradas." };
}

// ../web-six-cards/frontend/src/price-structure-history.ts
function mergeStructureMinutes(older, live) {
  if (!live?.sessions.length) return older;
  const first = live.sessions[0].openMs;
  return { sessions: [...older.sessions.filter((s) => s.openMs < first), ...live.sessions], minutes: [...older.minutes.filter((b) => b.timestampMs < first), ...live.minutes] };
}

// ../web-six-cards/frontend/src/price-structure.ts
var PRICE_STRUCTURE_RULE = Object.freeze({ atrPeriod: 14, pivotLeft: 2, pivotRight: 2, pivotLookback: 100, thresholdAtr: 0, minimumBars: 100, seedTolerance: 5e-3 });
var PRICE_INTERVALS = [15, 60, 240, 1440, 10080];
var names = { 15: ["15 MINUTOS", "CERCANA"], 60: ["1 HORA", "INTERMEDIA"], 240: ["4 HORAS", "AMPLIA"], 1440: ["DIARIO", "PRINCIPAL"], 10080: ["SEMANAL", "MAYOR"] };
var finite3 = (n) => typeof n === "number" && Number.isFinite(n);
var day2 = new Intl.DateTimeFormat("en-CA", { timeZone: "America/New_York" });
var validDate = (s) => /^\d{4}-\d{2}-\d{2}$/.test(s) && Number.isFinite(Date.parse(s + "T12:00:00Z")) && (/* @__PURE__ */ new Date(s + "T12:00:00Z")).toISOString().slice(0, 10) === s;
function priceStructureBars(history, interval, now) {
  const fail = (reason) => ({ bars: [], gaps: 0, reason });
  if (!history || !finite3(now) || !Array.isArray(history.minutes) || !Array.isArray(history.sessions)) return fail("Sin historial RTH verificable.");
  const sessions = [...history.sessions].sort((a, b) => a.openMs - b.openMs);
  if (sessions.some((s, i) => !validDate(s.date) || ![s.openMs, s.closeMs].every(finite3) || s.openMs % 6e4 !== 0 || s.closeMs % 6e4 !== 0 || s.closeMs <= s.openMs || s.closeMs - s.openMs > 864e5 || day2.format(s.openMs) !== s.date || i > 0 && (s.openMs < sessions[i - 1].closeMs || s.date === sessions[i - 1].date))) return fail("Calendario de sesiones inv\xE1lido o duplicado.");
  const minutes = /* @__PURE__ */ new Map();
  for (const b of history.minutes) {
    if (!finite3(b.timestampMs)) return fail("Minuto sin fecha v\xE1lida.");
    if (b.timestampMs + 6e4 > now) continue;
    if (![b.open, b.high, b.low, b.close, b.volume].every(finite3) || b.timestampMs % 6e4 !== 0 || Math.min(b.open, b.low, b.close) <= 0 || b.high < Math.max(b.open, b.close, b.low) || b.low > Math.min(b.open, b.close) || b.volume < 0 || minutes.has(b.timestampMs)) return fail("Minutos inv\xE1lidos o duplicados; sin sustituciones.");
    minutes.set(b.timestampMs, b);
  }
  let bars = [], gaps = 0;
  for (const session of sessions) {
    if (session.openMs > now) continue;
    const duration = interval === 1440 ? session.closeMs - session.openMs : interval * 6e4;
    for (let start = session.openMs; start < session.closeMs; start += duration) {
      const end = Math.min(start + duration, session.closeMs);
      if (end > now) break;
      let open = 0, high = -Infinity, low = Infinity, close = 0, volume = 0, complete = true;
      for (let t = start; t < end; t += 6e4) {
        const b = minutes.get(t);
        if (!b) {
          complete = false;
          break;
        }
        if (t === start) open = b.open;
        high = Math.max(high, b.high);
        low = Math.min(low, b.low);
        close = b.close;
        volume += b.volume;
      }
      if (!complete || !finite3(volume)) {
        gaps++;
        bars = [];
        continue;
      }
      bars.push({ timestampMs: start, endMs: end, sessionDate: session.date, open, high, low, close, volume });
    }
  }
  return { bars, gaps, reason: bars.length ? "" : gaps ? "Faltan minutos en la \xFAltima vela cerrada." : "Sin velas completas de esta temporalidad." };
}
function priceStructureAtr(bars) {
  const n = PRICE_STRUCTURE_RULE.atrPeriod;
  if (bars.length < n + 1) return null;
  let atr = 0;
  for (let i = 1; i < bars.length; i++) {
    const b = bars[i], previous = bars[i - 1].close;
    const tr = Math.max(b.high - b.low, Math.abs(b.high - previous), Math.abs(b.low - previous));
    if (!finite3(tr) || tr < 0) return null;
    if (i <= n) {
      atr += tr / n;
    } else atr = (atr * (n - 1) + tr) / n;
  }
  return finite3(atr) && atr > 0 ? atr : null;
}
function priceStructurePivots(bars) {
  const { pivotLeft: left, pivotRight: right, pivotLookback: lookback } = PRICE_STRUCTURE_RULE;
  const pivots = [];
  for (let i = Math.max(left, bars.length - lookback); i < bars.length - right; i++) {
    const neighbours = bars.slice(i - left, i + right + 1).filter((_, j) => j !== left);
    const high = neighbours.every((b) => bars[i].high > b.high), low = neighbours.every((b) => bars[i].low < b.low);
    if (high === low) continue;
    const pivot = { kind: high ? "high" : "low", price: high ? bars[i].high : bars[i].low, index: i, at: bars[i].timestampMs, confirmedMs: bars[i + right].endMs };
    const last = pivots.at(-1);
    if (last?.kind === pivot.kind) {
      if (pivot.kind === "high" ? pivot.price > last.price : pivot.price < last.price) pivots[pivots.length - 1] = pivot;
    } else pivots.push(pivot);
  }
  return pivots;
}
function classifyPriceStructure(series, interval, now) {
  const bars = series.bars, last = bars.at(-1), atr = priceStructureAtr(bars), pivots = priceStructurePivots(bars);
  const highs = pivots.filter((p) => p.kind === "high").slice(-2), lows = pivots.filter((p) => p.kind === "low").slice(-2);
  const card = { interval, name: names[interval][0], role: names[interval][1], side: "SIN DATOS", reason: series.reason, bars: bars.length, atr, highChange: null, lowChange: null, highs, lows, lastClose: last?.close ?? null, endMs: last?.endMs ?? null, reference: !!last && day2.format(last.endMs) !== day2.format(now), gaps: series.gaps, invalidated: false, seedDifference: null, lastDuration: last ? (last.endMs - last.timestampMs) / 6e4 : null };
  if (!last || atr == null) {
    card.reason ||= "Se necesitan 15 velas cerradas continuas para ATR14.";
    return card;
  }
  if (highs.length < 2 || lows.length < 2) {
    card.reason = "Faltan dos m\xE1ximos y dos m\xEDnimos confirmados en las \xFAltimas 100 velas.";
    return card;
  }
  card.highChange = (highs[1].price - highs[0].price) / atr;
  card.lowChange = (lows[1].price - lows[0].price) / atr;
  const tau = PRICE_STRUCTURE_RULE.thresholdAtr;
  const call = card.highChange > tau && card.lowChange > tau, put = card.highChange < -tau && card.lowChange < -tau;
  const after = Math.max(highs[1].index, lows[1].index) + 1;
  card.invalidated = call ? bars.slice(after).some((b) => b.close < lows[1].price - tau * atr) : put ? bars.slice(after).some((b) => b.close > highs[1].price + tau * atr) : false;
  card.side = card.invalidated ? "MIXTA" : call ? "CALL" : put ? "PUT" : "MIXTA";
  card.reason = card.invalidated ? "Un cierre rompi\xF3 el \xFAltimo extremo de soporte de la estructura." : call ? "M\xE1ximos y m\xEDnimos confirmados ascendentes." : put ? "M\xE1ximos y m\xEDnimos confirmados descendentes." : "Los m\xE1ximos y m\xEDnimos no definen la misma direcci\xF3n.";
  return card;
}
function priceStructureCards(s, symbol, active, now = Date.now(), history = s?.chartHistory, longHistory) {
  const own = active && s?.symbol === symbol && finite3(s.publishedMs) && s.publishedMs <= now && now - s.publishedMs <= 15e3;
  const minutes = longHistory?.symbol === symbol && longHistory.asOfMs <= now && day2.format(longHistory.asOfMs) === day2.format(now) ? mergeStructureMinutes(longHistory.intraday, s?.chartHistory) : history;
  return PRICE_INTERVALS.map((interval) => {
    if (!own) return classifyPriceStructure({ bars: [], gaps: 0, reason: !active ? "Data desconectada." : s?.symbol !== symbol ? "Historial de otro activo." : "Publicaci\xF3n sin vigencia." }, interval, now);
    const series = interval === 1440 || interval === 10080 ? longStructureBars(longHistory, symbol, interval, now) : priceStructureBars(minutes, interval, now);
    const card = classifyPriceStructure(series, interval, now);
    if (series.bars.length > 0 && series.bars.length < PRICE_STRUCTURE_RULE.minimumBars) {
      card.side = "SIN DATOS";
      card.reason = "Estabilizando historial: " + series.bars.length + "/100 velas cerradas.";
    }
    if (series.bars.length >= PRICE_STRUCTURE_RULE.minimumBars && card.atr != null) {
      const alternative = priceStructureAtr(series.bars.slice(14));
      card.seedDifference = alternative == null ? null : Math.abs(alternative / card.atr - 1);
      if (card.seedDifference == null || card.seedDifference > PRICE_STRUCTURE_RULE.seedTolerance) {
        card.side = "SIN DATOS";
        card.reason = "ATR sensible a su inicio; esperando m\xE1s historial.";
      }
    }
    if (series.bars.length) card.indicators = structuralIndicators(series.bars, interval, card.atr, symbol + ":" + interval);
    return card;
  });
}

// ../web-six-cards/frontend/src/saved-price-structure.ts
var STRUCTURE_VERSION = "price-structure-five-wilder14-pivots2-v1";
var date = new Intl.DateTimeFormat("en-CA", { timeZone: "America/New_York" });
function capturePriceStructure(frame, symbol, capturedMs, history, sha = null) {
  const valid = history?.symbol === symbol && history.asOfMs <= capturedMs && date.format(history.asOfMs) === date.format(capturedMs) ? history : void 0;
  return {
    version: STRUCTURE_VERSION,
    symbol,
    capturedMs,
    publishedMs: frame.publishedMs,
    historyAsOfMs: valid?.asOfMs ?? null,
    historySha256: valid ? sha : null,
    cards: priceStructureCards(frame, symbol, true, capturedMs, frame.chartHistory, valid)
  };
}

// ../web-six-cards/desktop-writer/price-structure-capture.ts
var input = JSON.parse(readFileSync(0, "utf8"));
var result = {};
for (const [symbol, frame] of Object.entries(input.frames)) {
  let history, sha = null;
  try {
    const file = input.histories[symbol], before = statSync(file, { bigint: true });
    if (before.mtimeNs > BigInt(input.capturedMs) * 1000000n) throw Error("Later acquisition");
    const bytes = readFileSync(file), after = statSync(file, { bigint: true });
    if (before.mtimeNs !== after.mtimeNs || before.ino !== after.ino || before.size !== after.size) throw Error("Changed cache");
    history = JSON.parse(bytes.toString("utf8"));
    sha = createHash("sha256").update(bytes).digest("hex");
  } catch {
  }
  result[symbol] = capturePriceStructure(frame, symbol, input.capturedMs, history, sha);
}
process.stdout.write(JSON.stringify(result));
