"""Immutable evidence and bounded multi-symbol outcome collection."""
from dataclasses import dataclass, asdict
import hashlib
import json
import math
import re
import sqlite3
from datetime import date
from contextlib import nullcontext

VERSION = 'snipers-learning-v2'


class FrozenFeatures(dict):
    def immutable(self, *args, **kwargs):
        raise TypeError('FROZEN_FEATURE_EVIDENCE')
    __setitem__ = __delitem__ = clear = pop = popitem = setdefault = update = __ior__ = immutable


def number(x):
    return isinstance(x, (int, float)) and not isinstance(x, bool) and math.isfinite(x)


@dataclass(frozen=True)
class ClockPolicy:
    lead_ms: int = 1000
    price_age_ms: int = 15000
    publication_age_ms: int = 3000

    def __post_init__(self):
        if not 0 <= self.lead_ms <= 1000 or not 1 <= self.price_age_ms <= 15000 or not 1 <= self.publication_age_ms <= 3000:
            raise ValueError('INVALID_CLOCK_POLICY')

    def checked(self, source, received, age):
        if not all(number(v) and v >= 0 for v in (source, received)):
            return None
        if source - received > self.lead_ms or received - source > age:
            return None
        # Never rewrite provider time: persist it alongside this bounded effective time.
        return min(source, received)


@dataclass(frozen=True)
class Reading:
    id: str
    symbol: str
    session: str
    phase: str
    at: int
    provider_ms: int
    price: float
    direction: int
    target: float | None
    stop: float | None
    horizon_ms: int
    session_end: int
    features: dict
    quality: tuple
    baseline_call: float | None
    provenance_json: str = '{}'

    def __post_init__(self):
        object.__setattr__(self,'features',FrozenFeatures(self.features))


def freeze(frame, horizon_ms=300000, policy=ClockPolicy()):
    """Contract: calendar bounds supplied by the existing authoritative calendar adapter.

    Premarket and RTH are independent partitions. Missing options never become votes.
    Returns diagnostic quality instead of silently dropping rejected frames.
    """
    f = json.loads(json.dumps(frame, allow_nan=False))
    at = f.get('received_ms')
    if not isinstance(at, int) or isinstance(at, bool) or at < 0:
        raise ValueError('INVALID_RECEIVE_TIME')
    symbol = f.get('symbol', '')
    if not re.fullmatch(r'[A-Z][A-Z.]{0,15}', symbol):
        raise ValueError('INVALID_SYMBOL')
    if horizon_ms not in (300000, 900000, 1800000):
        raise ValueError('INVALID_HORIZON')
    c = f.get('calendar', {})
    bounds = [c.get(k) for k in ('premarket_open_ms', 'rth_open_ms', 'close_ms')]
    if not all(isinstance(v, int) and not isinstance(v, bool) for v in bounds) or not bounds[0] < bounds[1] < bounds[2]:
        raise ValueError('INVALID_CALENDAR')
    session = c.get('date', '')
    if not re.fullmatch(r'\d{4}-\d{2}-\d{2}', session):
        raise ValueError('INVALID_SESSION')
    date.fromisoformat(session)
    pre, opening, close = bounds
    phase = 'PRE' if pre <= at < opening else 'RTH' if opening <= at < close else 'CLOSED'
    end = opening if phase == 'PRE' else close
    reasons = []
    if phase == 'CLOSED':
        reasons.append('OUTSIDE_SESSION')
    if f.get('connected') is not True:
        reasons.append('DISCONNECTED')
    if policy.checked(f.get('published_ms'), at, policy.publication_age_ms) is None:
        reasons.append('STALE_PUBLICATION')
    tick = f.get('tick', {})
    effective = policy.checked(tick.get('at'), at, policy.price_age_ms)
    price = tick.get('price')
    start = pre if phase == 'PRE' else opening
    if tick.get('symbol') != symbol or effective is None or effective < start or not number(price) or price <= 0:
        reasons.append('NO_FRESH_OWN_PRICE')
    if at + horizon_ms >= end:
        reasons.append('HORIZON_CROSSES_SESSION')
    direction = f.get('direction')
    if isinstance(direction, bool) or direction not in (-1, 1):
        reasons.append('NO_DIRECTION')
    features = {}
    for key, evidence in sorted(f.get('features', {}).items()):
        if not re.fullmatch(r'[a-z][a-z0-9_]{0,63}', key):
            raise ValueError('INVALID_FEATURE_NAME')
        value = evidence.get('value')
        valid = evidence.get('symbol') == symbol and evidence.get('phase') == phase and evidence.get('complete') is True
        valid &= policy.checked(evidence.get('at'), at, policy.publication_age_ms) is not None
        features[key] = value if valid and number(value) else None
    if len(features) > 64:
        raise ValueError('TOO_MANY_FEATURES')
    if not any(v is not None for v in features.values()):
        reasons.append('NO_FEATURE_EVIDENCE')
    target, stop = f.get('target'), f.get('stop')
    if target is not None or stop is not None:
        if not all(number(v) and v > 0 for v in (price, target, stop)) or direction not in (-1, 1) or (target-price)*direction <= 0 or (price-stop)*direction <= 0:
            reasons.append('INVALID_ROUTE_GEOMETRY')
    baseline = f.get('baseline_call')
    if not number(baseline) or not 0 <= baseline <= 1:
        baseline = None
    evidence=f.get('baseline_evidence')
    if not isinstance(evidence,dict):
        evidence={'kind':'legacy_unverified','status':'missing' if baseline is None else 'unverified_definition'}
    elif evidence.get('kind')!='probability' or evidence.get('symbol')!=symbol or evidence.get('phase')!=phase or policy.checked(evidence.get('at'),at,policy.publication_age_ms) is None or evidence.get('horizon_ms')!=horizon_ms or evidence.get('endpoint')!='upward-price-endpoint' or not evidence.get('source') or not evidence.get('definition') or evidence.get('calibrated') is not True:
        baseline=None
    provenance=json.dumps({'version':'v2-capture-provenance-v1','captured_ms':at,'published_ms':f.get('published_ms'),
        'direction_source':f.get('direction_source','unspecified'),'original_feature_evidence':f.get('features',{}),
        'baseline_evidence':evidence,'baseline_usable':baseline is not None and evidence.get('kind')=='probability'},sort_keys=True,allow_nan=False)
    identity = hashlib.sha256(json.dumps([VERSION, symbol, session, phase, at, horizon_ms], separators=(',', ':')).encode()).hexdigest()
    return Reading(identity, symbol, session, phase, at, tick.get('at'), price, direction, target, stop, horizon_ms, end, features, tuple(reasons), baseline,provenance)


class Store:
    """Separate v2 ledger; outcomes survive view changes and process restarts.

    Excursions describe received price samples, never certify unobserved intrabar highs/lows.
    A gap makes the path incomplete even if the final price is received.
    """
    def __init__(self, path, capacity=20, policy=ClockPolicy()):
        if not 1 <= capacity <= 20:
            raise ValueError('INVALID_CAPACITY')
        self.capacity, self.policy = capacity, policy
        self.db = sqlite3.connect(path)
        self.db.execute('PRAGMA foreign_keys=ON')
        tables = {r[0] for r in self.db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        if tables and 'v2_meta' not in tables:
            self.db.close()
            raise ValueError('FOREIGN_DATABASE')
        if 'v2_meta' in tables and self.db.execute('SELECT version FROM v2_meta').fetchall() != [(VERSION,)]:
            self.db.close()
            raise ValueError('FOREIGN_VERSION')
        self.db.executescript('''
          CREATE TABLE IF NOT EXISTS v2_meta(version TEXT PRIMARY KEY);
          CREATE TABLE IF NOT EXISTS v2_readings(id TEXT PRIMARY KEY, symbol TEXT, session TEXT, phase TEXT, at INTEGER, deadline INTEGER, end INTEGER, payload TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS v2_outcomes(id TEXT PRIMARY KEY REFERENCES v2_readings(id), status TEXT, last_ms INTEGER, mfe REAL, mae REAL, final_bps REAL, path_complete INTEGER, first_event TEXT, event_ms INTEGER);
          CREATE INDEX IF NOT EXISTS v2_pending ON v2_outcomes(status,last_ms);
        ''')
        self.db.execute('INSERT OR IGNORE INTO v2_meta VALUES(?)', (VERSION,))
        self.db.commit()

    def close(self):
        self.db.close()

    def required_symbols(self):
        return [r[0] for r in self.db.execute("SELECT DISTINCT r.symbol FROM v2_readings r JOIN v2_outcomes o USING(id) WHERE o.status='pending' ORDER BY r.symbol")]

    def record(self, reading, _commit=True):
        if not isinstance(reading, Reading) or not isinstance(reading.at,int) or reading.at < 0 or reading.horizon_ms not in (300000,900000,1800000):
            raise ValueError('INVALID_READING')
        if not reading.quality and (reading.phase not in ('PRE','RTH') or not number(reading.price) or reading.price<=0 or reading.direction not in (-1,1) or reading.at+reading.horizon_ms>=reading.session_end):
            raise ValueError('INVALID_ELIGIBLE_READING')
        payload = json.dumps(asdict(reading), sort_keys=True, allow_nan=False)
        with (self.db if _commit else nullcontext()):
            previous = self.db.execute('SELECT payload FROM v2_readings WHERE id=?', (reading.id,)).fetchone()
            if previous:
                if previous[0] != payload:
                    raise ValueError('IMMUTABLE_READING_CONFLICT')
                return False
            if not reading.quality and reading.symbol not in self.required_symbols() and len(self.required_symbols()) >= self.capacity:
                raise ValueError('FOLLOWUP_CAPACITY_FULL')
            self.db.execute('INSERT INTO v2_readings VALUES(?,?,?,?,?,?,?,?)', (reading.id, reading.symbol, reading.session, reading.phase, reading.at, reading.at+reading.horizon_ms, reading.session_end, payload))
            self.db.execute('INSERT INTO v2_outcomes VALUES(?,?,?,?,?,?,?,?,?)', (reading.id, 'rejected' if reading.quality else 'pending', reading.at, 0, 0, None, 1, None, None))
        return True

    def advance(self, symbol, session, phase, price, provider_ms, received_ms):
        effective = self.policy.checked(provider_ms, received_ms, self.policy.price_age_ms)
        if effective is None or not number(price) or price <= 0:
            return 0
        rows = self.db.execute("SELECT r.id,r.payload,r.deadline,r.end,o.last_ms,o.mfe,o.mae,o.path_complete,o.first_event,o.event_ms FROM v2_readings r JOIN v2_outcomes o USING(id) WHERE o.status='pending' AND r.symbol=? AND r.session=? AND r.phase=?", (symbol, session, phase)).fetchall()
        changed = 0
        with self.db:
            for id_, encoded, deadline, end, last, mfe, mae, complete, event, event_ms in rows:
                if effective <= last or effective >= end:
                    continue
                r = json.loads(encoded)
                if effective > deadline + self.policy.price_age_ms:
                    continue
                bps = (price/r['price']-1)*10000*r['direction']
                complete = bool(complete and effective-last <= self.policy.price_age_ms)
                # Grace price may measure endpoint; it must not extend the path beyond deadline.
                if effective <= deadline:
                    mfe, mae = max(mfe, bps), min(mae, bps)
                    if not event and r['target'] is not None:
                        if (price-r['target'])*r['direction'] >= 0:
                            event, event_ms = 'target', effective
                        elif (price-r['stop'])*r['direction'] <= 0:
                            event, event_ms = 'invalidated', effective
                status = 'observed' if effective >= deadline else 'pending'
                self.db.execute('UPDATE v2_outcomes SET status=?,last_ms=?,mfe=?,mae=?,final_bps=?,path_complete=?,first_event=?,event_ms=? WHERE id=?', (status, effective, mfe, mae, bps if status=='observed' else None, complete, event, event_ms, id_))
                changed += 1
        return changed

    def expire(self, now):
        if not isinstance(now,int) or isinstance(now,bool) or now<0:
            raise ValueError('INVALID_EXPIRE_TIME')
        with self.db:
            self.db.execute("UPDATE v2_outcomes SET status='missing',path_complete=0 WHERE status='pending' AND id IN (SELECT id FROM v2_readings WHERE deadline+? < ? OR end <= ?)", (self.policy.price_age_ms, now, now))

    def rows(self):
        rows = self.db.execute('SELECT r.payload,o.status,o.last_ms,o.mfe,o.mae,o.final_bps,o.path_complete,o.first_event,o.event_ms FROM v2_readings r JOIN v2_outcomes o USING(id) ORDER BY r.at,r.id')
        return [{**json.loads(p), 'status': status, 'outcome_ms': at, 'mfe_bps': mfe, 'mae_bps': mae, 'return_bps': ret, 'path_complete': bool(complete), 'first_event': event, 'event_ms': event_ms} for p,status,at,mfe,mae,ret,complete,event,event_ms in rows]


def correlation(left, right, minimum=20, coverage=.8):
    """Aligned return timestamps only. No forward fill and no correlation-as-causation."""
    common = sorted(set(left) & set(right))
    pairs = [(left[t], right[t]) for t in common if number(left[t]) and number(right[t])]
    if len(pairs) < minimum or len(pairs)/max(1,len(set(left)|set(right))) < coverage:
        return None
    a, b = zip(*pairs)
    am, bm = sum(a)/len(a), sum(b)/len(b)
    numerator = sum((x-am)*(y-bm) for x,y in pairs)
    denom = math.sqrt(sum((x-am)**2 for x in a)*sum((y-bm)**2 for y in b))
    return max(-1, min(1,numerator/denom)) if denom > 0 else None


def basket_context(weights, contributions, holdings_ms, now):
    """Official weight percent + independently verified return fractions by horizon.

    Adapter must certify each contribution using own current price and baseline.
    Unknown members do not become zero returns and horizons never substitute each other.
    Output is context for a candidate, not an additional direction vote.
    """
    if not number(holdings_ms) or not 0 <= now-holdings_ms <= 7*86400000:
        raise ValueError('STALE_HOLDINGS')
    if not weights or any(not number(w) or not 0<w<=100 for w in weights.values()) or sum(weights.values())>100:
        raise ValueError('INVALID_WEIGHTS')
    results={}
    for horizon in ('previous_close','premarket_open','five_minutes'):
        values=contributions.get(horizon,{})
        valid={k:v for k,v in values.items() if k in weights and number(v)}
        coverage=sum(weights[k] for k in valid)
        available=coverage>=40 and coverage>=.8*sum(weights.values())
        results[horizon]={'coverage_pct':coverage,'available':available,
                          'contribution_points':sum(weights[k]*v for k,v in valid.items()) if available else None,
                          'supporting':sorted(k for k,v in valid.items() if v>0),
                          'opposing':sorted(k for k,v in valid.items() if v<0)}
    return results


def health_report(rows):
    """Quality denominators remain visible; rejected records never count as learning wins."""
    from collections import Counter
    partitions={}
    for r in rows:
        key=f"{r['symbol']}:{r['phase']}:{r['horizon_ms']}"
        bucket=partitions.setdefault(key,{'records':0,'eligible':0,'statuses':Counter(),'reasons':Counter(),'complete_paths':0})
        bucket['records']+=1
        bucket['eligible']+=int(not r['quality'])
        bucket['statuses'][r['status']]+=1
        bucket['reasons'].update(r['quality'])
        bucket['complete_paths']+=int(r['status']=='observed' and r['path_complete'])
    return {'schema':VERSION,'partitions':partitions,'automatic_model_updates':False}
