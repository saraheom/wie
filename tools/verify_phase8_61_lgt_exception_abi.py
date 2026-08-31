from pathlib import Path

root = Path(__file__).resolve().parents[1]
interface = (root / 'wie_lgt/src/runtime/java/interface.rs').read_text()
workflow = (root / '.github/workflows/ios-testflight.yml').read_text()
config = (root / 'wie_app/tauri.conf.json').read_text()
marker = (root / 'wie_web/public/phase8_61_build.txt').read_text().strip()

required = [
    'PHASE8_61_LGT_IS_ASSIGNABLE_ENTRY',
    'PHASE8_61_LGT_IS_ASSIGNABLE_RESULT',
    'PHASE8_61_LGT_IS_ASSIGNABLE_INVALID_NAME',
    'PHASE8_61_LGT_THROW_EXCEPTION',
    'java_is_class_assignable',
    'jvm.is_type_assignable',
    'Err(WieError::JavaException(ptr_exception))',
]
for token in required:
    if token not in interface:
        raise SystemExit(f'Missing Phase 8.61 LGT ABI token: {token}')

if 'async fn java_rethrow_exception' in interface:
    raise SystemExit('Obsolete Phase 8.60 rethrow/pop implementation is still present')

if '"version": "0.1.61"' not in config:
    raise SystemExit('Tauri version is not 0.1.61')

expected = 'PHASE8_61_BUILD_SENTINEL=WIPI_PLAYER_0.1.61_OZ_LGT_EXCEPTION_ABI_ALIGNMENT'
if marker != expected:
    raise SystemExit(f'Unexpected Phase 8.61 build marker: {marker!r}')

for token in ['PHASE8_61_LGT_IS_ASSIGNABLE_ENTRY', 'PHASE8_61_LGT_THROW_EXCEPTION', expected]:
    if token not in workflow:
        raise SystemExit(f'TestFlight workflow does not verify: {token}')

print('Phase 8.61 LGT exception/type ABI alignment verified')
