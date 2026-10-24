"""Prepared bridge from serialized LiveState + frozen direction evidence.

Not registered with Tauri. Caller must use the authoritative calendar and the
existing direction reader; this adapter never recalculates the production score.
"""
from .core import freeze, number


def from_live(state, decision, received_ms, horizon_ms=300000):
    symbol=state.get('symbol')
    sessions=[s for s in state.get('chartHistory',{}).get('sessions',[]) if s.get('date')==state.get('metrics',{}).get('session')]
    if len(sessions)!=1:
        raise ValueError('MISSING_AUTHORITATIVE_CALENDAR')
    session=sessions[0]
    if decision.get('symbol')!=symbol:
        raise ValueError('FOREIGN_DIRECTION_READING')
    features={}
    for evidence in decision.get('features',[]):
        name=evidence['name']
        if name in features:
            raise ValueError('DUPLICATE_FEATURE')
        features[name]={k:evidence.get(k) for k in ('symbol','phase','value','complete','at','source','definition','window_start_ms','window_end_ms')}
    opening=session['openMs']
    phase='PRE' if received_ms<opening else 'RTH'
    tick=state.get('ticks',{}).get(symbol) or {}
    frame={'symbol':symbol,'received_ms':received_ms,'published_ms':state.get('publishedMs'),
           'connected':state.get('stocks',{}).get('phase')=='streaming',
           'calendar':{'date':session['date'],'premarket_open_ms':opening-19800000,'rth_open_ms':opening,'close_ms':session['closeMs']},
           'tick':{'symbol':tick.get('symbol'),'price':tick.get('price'),'at':tick.get('tradeMs')},
           'direction':decision.get('direction'),'target':decision.get('target'),'stop':decision.get('stop'),
           'features':features,'baseline_call':decision.get('baseline_call'),
           'baseline_evidence':decision.get('baseline_evidence'),
           'direction_source':decision.get('direction_source','unspecified')}
    reading=freeze(frame,horizon_ms)
    # Never silently attach another phase's headline or a stale decision to a current tick.
    decision_at=decision.get('at')
    age=received_ms-decision_at if number(decision_at) else None
    if not number(age) or not 0<=age<=3000 or decision.get('phase')!=phase:
        from dataclasses import replace
        reading=replace(reading,quality=reading.quality+('STALE_OR_FOREIGN_DECISION',))
    return reading
