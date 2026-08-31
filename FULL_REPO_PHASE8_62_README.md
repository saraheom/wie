# Phase 8.62 — OZ LGT black-screen/network localization

Based on the Phase 8.61 test where OZ remains alive on a black screen for ~128 seconds. This phase intentionally does not fabricate server success yet. It lowers the native-loop diagnostic threshold for exact AID 00026DBF / PID PD112525 and adds explicit Java/WIPI network-path logging so the next run can distinguish CPU loop vs carrier-network validation vs another wait path.
