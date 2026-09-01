from pathlib import Path
root=Path(__file__).resolve().parents[1]
runtime=(root/'wie_jvm_support/src/runtime.rs').read_text()
fileimpl=(root/'wie_jvm_support/src/runtime/file.rs').read_text()
assert 'PHASE8_68_OZ_RUNTIME_OPEN_ENTRY' in runtime
assert 'PHASE8_68_OZ_RUNTIME_OPEN_COMPLETE' in runtime
assert 'PHASE8_68_OZ_FILEIMPL_EXISTS_BEGIN' in fileimpl
assert 'PHASE8_68_OZ_FILEIMPL_EXISTS_RETURN' in fileimpl
assert 'PHASE8_67_OZ_DIRECT_BINARY_MOD' in (root/'wie_lgt/src/emulator.rs').read_text()
assert '"version": "0.1.68"' in (root/'wie_app/tauri.conf.json').read_text()
assert (root/'wie_web/public/phase8_68_build.txt').read_text().strip() == 'PHASE8_68_BUILD_SENTINEL=WIPI_PLAYER_0.1.68_OZ_RANDOMACCESSFILE_VFS_LOCALIZATION'
print('Phase 8.68 verification passed')
