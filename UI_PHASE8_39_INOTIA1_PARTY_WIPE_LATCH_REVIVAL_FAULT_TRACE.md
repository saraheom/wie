# Phase 8.39 test notes

- Normal gameplay and cash shop must remain identical to Phase 8.37.
- Emergency missing-prayer flow must stay latched across the title's state-14 -> state-6 reconnect.
- CLEAR during that latched flow restores death state 11 / selection 0 and is still forwarded normally.
- Blessed-revival crash tracing runs only on an already-fatal ARM memory/PC fault.
