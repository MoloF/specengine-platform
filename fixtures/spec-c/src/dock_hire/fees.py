"""Ride fees: the late-return fee and the day cap."""

from datetime import timedelta

FREE_RIDE = timedelta(minutes=30)
QUARTER = timedelta(minutes=15)
LATE_FEE = 1.00
DAY_CAP = 12.00


def late_fee(ride: timedelta) -> float:
    """One fee per started quarter hour past the free ride."""
    if ride <= FREE_RIDE:
        return 0.0
    quarters = -(-(ride - FREE_RIDE) // QUARTER)
    return quarters * LATE_FEE


def day_total(fees: list[float]) -> float:
    """The fees of one rider in one day, never above the cap."""
    return min(sum(fees), DAY_CAP)
