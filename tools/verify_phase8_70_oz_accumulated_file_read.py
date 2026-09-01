from pathlib import Path
root=Path(__file__).resolve().parents[1]
f=(root/'wie_jvm_support/src/runtime/file.rs').read_text()
for marker in ['PHASE8_70_OZ_FILE_READ_ACCUM_BEGIN','PHASE8_70_OZ_FILE_READ_CHUNK_BEGIN','PHASE8_70_OZ_FILE_READ_CHUNK_RETURN','PHASE8_70_OZ_FILE_READ_ACCUM_RETURN']:
    assert marker in f
assert 'MAX_VFS_READ_CHUNK: usize = 64 * 1024' in f
assert 'while total_read < request_len' in f
assert 'Ok(total_read)' in f
assert '"version": "0.1.70"' in (root/'wie_app/tauri.conf.json').read_text()
assert (root/'wie_web/public/phase8_70_build.txt').read_text().strip() == 'PHASE8_70_BUILD_SENTINEL=WIPI_PLAYER_0.1.70_OZ_ACCUMULATED_CHUNKED_FILE_READ'
wf=(root/'.github/workflows/ios-testflight.yml').read_text()
assert 'phase8_70_build.txt' in wf
assert 'PHASE8_70_BUILD_SENTINEL=WIPI_PLAYER_0.1.70_OZ_ACCUMULATED_CHUNKED_FILE_READ' in wf
assert 'expected 0.1.70' in wf
print('Phase 8.70 verifier passed')
