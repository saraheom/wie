from pathlib import Path

src = Path('wie_lgt/src/runtime/java/interface.rs').read_text(encoding='utf-8')
start = src.index('async fn java_initialize_class(')
end = src.index('\nasync fn java_get_array_type', start)
body = src[start:end]

required = [
    'const CLASS_STATE_INITIALIZING: i32 = 4;',
    'const CLASS_STATE_INITIALIZED: i32 = 5;',
    'state == CLASS_STATE_INITIALIZED',
    'state == CLASS_STATE_INITIALIZING',
    'PHASE8_60_LGT_CLASS_INIT_BEGIN',
    'PHASE8_60_LGT_CLASS_INIT_REENTRANT',
    'PHASE8_60_LGT_CLASS_INIT_COMPLETE',
    'PHASE8_60_LGT_CLASS_INIT_CALLBACK_ERROR',
]
for token in required:
    if token not in body:
        raise SystemExit(f'Missing Phase 8.60 class-init token: {token}')

write_initializing = body.index('CLASS_STATE_INITIALIZING,', body.index('jvm.put_field('))
run_callback = body.index('core.run_function(callback, &[])')
write_initialized = body.rindex('CLASS_STATE_INITIALIZED,')
if not (write_initializing < run_callback < write_initialized):
    raise SystemExit('Class-init state transition order is invalid')

if 'if ready == 5' in body:
    raise SystemExit('Pre-8.60 single-state initialization guard still present')

print('Phase 8.60 LGT class-initialization re-entrancy repair verified')
print('state 4 write precedes callback; state 5 write follows callback')
