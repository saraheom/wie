from pathlib import Path
root=Path(__file__).resolve().parents[1]
emu=(root/'wie_lgt/src/emulator.rs').read_text()
java=(root/'wie_lgt/src/runtime/java.rs').read_text()
assert 'PHASE8_67_OZ_DIRECT_BINARY_MOD' in emu
assert 'jar_filename == "00026DBF.jar"' in emu
assert 'get("binary.mod")' in emu
assert 'PHASE8_66_OZ_FIND_RESOURCE_METADATA_BYPASS' not in java
assert '"version": "0.1.67"' in (root/'wie_app/tauri.conf.json').read_text()
assert (root/'wie_web/public/phase8_67_build.txt').read_text().strip() == 'PHASE8_67_BUILD_SENTINEL=WIPI_PLAYER_0.1.67_OZ_DIRECT_BINARY_MOD_BOOTSTRAP'
print('phase 8.67 verification passed')
