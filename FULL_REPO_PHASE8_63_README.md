# WIPI Player Phase 8.63 — OZ LGT SVC Hang Localization

This diagnostic phase preserves the Phase 8.60–8.62 compatibility work. The latest OZ log reaches LGT startup but then becomes silent before any Phase 8.62 network probe or NATIVE_LOOP warning. That pattern is consistent with entering a host-side SVC handler that does not return to the ARM engine.

For AID `00026DBF`, PID `PD112525` only, Phase 8.63 logs every ARM SVC boundary with `PHASE8_63_OZ_SVC_ENTRY` and a matching `PHASE8_63_OZ_SVC_RETURN`. The last ENTRY without a RETURN identifies the blocking service category and call-site registers. Execution semantics and network behavior are unchanged.
