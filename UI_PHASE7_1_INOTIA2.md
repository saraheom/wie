# Phase 7.1 — Inotia 2 compatibility + Korean import wording

## Korean wording

The Korean home-page import action now uses **+ 불러오기**.

## WIPI archive wrapper normalization

`wie_backend::extract_zip` now detects ZIPs where every file is inside one common top-level directory and strips exactly that wrapper directory before carrier detection. This allows original phone dumps such as Inotia 2 to be imported directly even when `__adf__` is not physically at the ZIP root. Nested game resource folders are preserved.

## WIPI memory reporting

The previous `MC_knlGetTotalMemory` / `MC_knlGetFreeMemory` stubs reported only 1 MiB despite WIE mapping a much larger ARM heap. They now conservatively report 16 MiB. This addresses later WIPI titles that abort with an in-game memory error after otherwise launching successfully.

The allocator itself is unchanged; this only corrects the guest-visible platform memory report.
