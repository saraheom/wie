# Phase 8.46 UI / TestFlight notes

- TestFlight marketing version: `0.1.46`.
- Inotia1 Settings > Diagnostics retains **Arm/Reset EXP Trace**.
- The watcher starts disarmed and is reset/armed by the button.
- 16-bit guest writer PC `0x001069c2` is suppressed from EXP diagnostics based on the Phase 8.45 field trace.
- Other callsites are capped at 24 retained events per PC+width, with four per exact address+PC+width.
- Normal Inotia1 cash shop: 10 records; first 8 proven utility items plus `힘의 조각` and `마법의 가지`.
- Removed normal-catalog tail: `흑기사의 투구`, `레게 스타일`, `번개 스타일`, `스텔스 가면`.
