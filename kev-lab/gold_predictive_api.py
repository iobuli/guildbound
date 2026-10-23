"""Small cached HTTP payload, separate from the large observatory event archive."""
import json
from functools import lru_cache
from pathlib import Path
from threadpoolctl import threadpool_limits

import gold_information_study as S
from gold_predictive import forecast_asof


@lru_cache(maxsize=1)
def data():
    return S.load()


@lru_cache(maxsize=2)
def predictive_report(asof=None):
    policy=json.loads((Path(__file__).parent/'gold-predictive-policy.json').read_text())
    with threadpool_limits(limits=4):
        return forecast_asof(data(),policy,asof)
