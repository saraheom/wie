# UI Phase 8.58 — OZ LGT Wide-Field Linker

No UI behavior changes. This phase repairs the LGT Java AOT class linker so sparse null/null field metadata slots following `J`/`D` fields are treated as the second 32-bit word of a 64-bit field. Diagnostic marker: `PHASE8_58_LGT_WIDE_FIELD_CONTINUATION`.
