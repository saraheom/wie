from pathlib import Path
root=Path(__file__).resolve().parents[1]
java=(root/'wie_lgt/src/runtime/java.rs').read_text()
core=(root/'wie_core_arm/src/core.rs').read_text()
wf=(root/'.github/workflows/ios-testflight.yml').read_text()
marker=(root/'wie_web/public/phase8_66_build.txt').read_text().strip()
assert 'PHASE8_66_OZ_FIND_RESOURCE_METADATA_BYPASS' in java
assert 'core.oz_svc_hang_diagnostics_enabled()' in java
assert 'core.write_return_value(&[0])?' in java
assert 'pub fn oz_svc_hang_diagnostics_enabled(&self) -> bool' in core
assert '"version": "0.1.66"' in (root/'wie_app/tauri.conf.json').read_text()
assert 'phase8_66_build.txt' in wf
assert 'PHASE8_66_BUILD_SENTINEL=WIPI_PLAYER_0.1.66_OZ_URLCLASSLOADER_METADATA_BYPASS' == marker
print('Phase 8.66 verifier passed')
