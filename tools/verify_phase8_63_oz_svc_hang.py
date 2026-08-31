from pathlib import Path
root = Path(__file__).resolve().parents[1]
assert 'PHASE8_63_OZ_SVC_ENTRY' in (root/'wie_core_arm/src/core.rs').read_text()
assert 'PHASE8_63_OZ_SVC_RETURN' in (root/'wie_core_arm/src/core.rs').read_text()
assert 'PHASE8_63_OZ_HANG_PROBE' in (root/'wie_lgt/src/emulator.rs').read_text()
assert (root/'wie_web/public/phase8_63_build.txt').read_text().strip() == 'PHASE8_63_BUILD_SENTINEL=WIPI_PLAYER_0.1.63_OZ_LGT_SVC_HANG_LOCALIZATION'
assert '"version": "0.1.63"' in (root/'wie_app/tauri.conf.json').read_text()
print('Phase 8.63 verification passed')
