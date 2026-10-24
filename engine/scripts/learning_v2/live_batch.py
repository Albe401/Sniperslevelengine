"""Bounded native-feed batch. Uses no network and never activates model weights."""
import json
import sys
from pathlib import Path
from .core import Store, VERSION, number
from .collector import Collector


def price_evidence(state, decision, now):
    """Learning-only feature; never supplies a production probability or trade signal."""
    sessions=[s for s in state.get('chartHistory',{}).get('sessions',[]) if s.get('date')==state.get('metrics',{}).get('session')]
    if len(sessions)!=1:
        return decision
    opening,close=sessions[0]['openMs'],sessions[0]['closeMs']
    phase='PRE' if opening-19800000<=now<opening else 'RTH' if opening<=now<close else 'CLOSED'
    start=(now//60000-6)*60000
    if phase=='CLOSED' or start<(opening-19800000 if phase=='PRE' else opening):
        return decision
    raw=state.get('minutes')
    rows=list(raw.values()) if isinstance(raw,dict) else state.get('chartHistory',{}).get('minutes',[])
    closes=[]
    for at in range(start,start+360000,60000):
        matches=[r for r in rows if r.get('timestampMs')==at]
        if len(matches)!=1:
            return decision
        b=matches[0]
        if not all(number(b.get(k)) for k in ('open','high','low','close','volume')) or min(b['open'],b['close'],b['low'])<=0 or b['volume']<0 or b['high']<max(b['open'],b['close'],b['low']) or b['low']>min(b['open'],b['close']):
            return decision
        closes.append(b['close'])
    change=(closes[-1]/closes[0]-1)*100
    out={**decision,'features':list(decision.get('features',[]))}
    out['features'].append({'name':'closed_price_change_5m_pct','symbol':state['symbol'],'phase':phase,'at':now,'value':change,'complete':True,
        'source':'six_contiguous_closed_one_minute_candles','definition':'100*(last_close/first_close-1)',
        'window_start_ms':start,'window_end_ms':start+360000})
    if out.get('direction') not in (-1,1) and change!=0:
        out['direction']=1 if change>0 else -1
        out['direction_source']='closed_price_change_5m_pct'
    return out


def process(directory, batch):
    store = Store(Path(directory) / 'snipers-learning-v2.sqlite')
    errors = []
    recorded = 0
    try:
        collector = Collector(store)
        now = batch['received_ms']
        store.expire(now)
        for item in batch['frames'][:21]:
            try:
                recorded += len(collector.ingest(item['state'], price_evidence(item['state'],item['decision'],now), now))
            except (ValueError, KeyError, TypeError) as error:
                errors.append({'symbol': item.get('state', {}).get('symbol'), 'reason': str(error)})
        counts = dict(store.db.execute('SELECT status,COUNT(*) FROM v2_outcomes GROUP BY status'))
        return {'version': VERSION, 'integrated': True, 'mode': 'PASSIVE',
                'automatic_activation': False, 'observed_ms': now, 'recorded': recorded,
                'counts': counts, 'required_symbols': collector.required_symbols(), 'errors': errors}
    finally:
        store.close()


if __name__ == '__main__':
    print(json.dumps(process(sys.argv[1], json.load(sys.stdin)), allow_nan=False))
