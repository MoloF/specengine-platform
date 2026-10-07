from datetime import timedelta

from dock_hire.fees import day_total, late_fee


def test_a_45_minute_ride_costs_one_quarter():
    assert late_fee(timedelta(minutes=45)) == 1.00


def test_the_day_cap_holds():
    assert day_total([1.00] * 14) == 12.00
