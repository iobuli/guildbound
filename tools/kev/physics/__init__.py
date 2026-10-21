"""Optional physics pass for Kev landmark animation.

Animation stays authored. `correct` runs ground contact, balance, momentum and
secondary motion on a sequence and returns the corrected frames plus diagnostics.
`PhysicsPass.step` is the same pass one frame at a time, with hit-stop and impulse
hooks for a combo system.

    from physics import correct, PhysicsPass
"""
from .sim import PhysicsPass, correct

__all__ = ["PhysicsPass", "correct"]
